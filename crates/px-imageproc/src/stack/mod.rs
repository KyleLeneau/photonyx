//! Whole-sequence reduction: turning N frames into one (ADR 007, Phase 3).
//!
//! The frames do not fit in memory, and reading one pixel at a time from N
//! files is hopeless, so the work is cut into blocks by
//! [`crate::runtime::BlockPlan`]: a band of rows is read from every frame at
//! once, reduced pixel by pixel across frames, and written to the output
//! before the next band is read. Memory is bounded by the block size, which
//! is chosen from the budget, and threads each hold one band.

pub mod normalize;
pub mod rejection;

use rayon::prelude::*;

use crate::error::ImageProcError;
use crate::frame::Frame;
use crate::runtime::{Block, ImageDims, Runtime};
use crate::sequence::FrameSet;
use crate::stats;

pub use normalize::{NormCoefficients, NormMethod, Normalization};
pub use rejection::{Bounds, Rejected, Rejection};

/// How a pixel's values across the sequence become one value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Method {
    /// Sum of all values. Saturates rather than wrapping.
    Sum,
    /// Per-pixel minimum across the sequence.
    Min,
    /// Per-pixel maximum across the sequence.
    Max,
    /// Median. No rejection needed — the median is already robust — which is
    /// what makes it right for master bias and dark frames.
    Median,
    /// Average of the values that survive `rejection`.
    Mean(Rejection),
}

impl Method {
    fn needs_rejection_scratch(&self) -> bool {
        matches!(self, Method::Mean(r) if !matches!(r, Rejection::None))
    }
}

/// Knobs for one stacking run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StackOptions {
    pub method: Method,
    /// Rescale the result so its non-zero values span [0, 1].
    ///
    /// Without this the result is clamped to [0, 1] instead. The two are
    /// mutually exclusive: rescaling a clamped image would stretch whatever
    /// the clamp already flattened.
    pub output_norm: bool,
    /// Also produce per-pixel counts of how many values were rejected from
    /// each tail. Costs two `u16` images worth of memory.
    pub rejection_maps: bool,
}

impl StackOptions {
    pub fn new(method: Method) -> Self {
        Self {
            method,
            output_norm: false,
            rejection_maps: false,
        }
    }

    pub fn output_norm(mut self, on: bool) -> Self {
        self.output_norm = on;
        self
    }

    pub fn rejection_maps(mut self, on: bool) -> Self {
        self.rejection_maps = on;
        self
    }

    fn rejection_map_count(&self) -> usize {
        if self.rejection_maps && self.method.needs_rejection_scratch() {
            2
        } else {
            0
        }
    }
}

/// A stacked frame and what it cost.
pub struct StackResult {
    pub frame: Frame,
    /// Values rejected across the whole image, by tail.
    pub rejected: Rejected,
    /// Per-pixel count of low-tail rejections, when requested.
    pub rejection_map_low: Option<Vec<u16>>,
    /// Per-pixel count of high-tail rejections, when requested.
    pub rejection_map_high: Option<Vec<u16>>,
    /// Blocks the run was divided into — the observable trace of the memory
    /// plan, useful in tests and reports.
    pub block_count: usize,
}

/// Something a block-scheduled reduction can read bands from.
///
/// Two implementations, and the difference between them is where the frames
/// live: a [`FrameSet`] reads each band off disk as it is needed, and a slice of
/// loaded [`Frame`]s copies it out of memory. The stacker does not care which —
/// what it needs is a uniform shape, a count, and a band at a time — and the
/// caller chooses on the memory budget (ADR 007 P11-T3). For a master light
/// whose warped frames already fit in the budget, writing them out only to read
/// them straight back is pure I/O.
pub trait BlockSource: Sync {
    fn frame_count(&self) -> usize;
    fn dims(&self) -> ImageDims;
    /// Reads one block of one frame into `out`, which holds exactly
    /// `block.pixel_count(dims)` samples.
    fn read_block_into(
        &self,
        index: usize,
        block: &Block,
        out: &mut [f32],
    ) -> Result<(), ImageProcError>;
    /// The header the output frame inherits.
    fn first_header(&self) -> Option<px_fits::header::Header>;
}

impl BlockSource for FrameSet {
    fn frame_count(&self) -> usize {
        self.len()
    }

    fn dims(&self) -> ImageDims {
        FrameSet::dims(self)
    }

    fn read_block_into(
        &self,
        index: usize,
        block: &Block,
        out: &mut [f32],
    ) -> Result<(), ImageProcError> {
        FrameSet::read_block_into(self, index, block, out)
    }

    fn first_header(&self) -> Option<px_fits::header::Header> {
        self.get(0).map(|f| f.header.clone())
    }
}

/// Frames already in memory, in sequence order.
///
/// Uniformity is checked on construction, for the same reason [`FrameSet`] checks
/// it: finding out halfway through a reduction that frame 40 is a different shape
/// is the bad outcome.
pub struct LoadedFrames<'a> {
    frames: &'a [Frame],
    dims: ImageDims,
}

impl<'a> LoadedFrames<'a> {
    pub fn new(frames: &'a [Frame]) -> Result<Self, ImageProcError> {
        let first = frames.first().ok_or(ImageProcError::EmptySequence)?;
        let dims = first.dims();
        for frame in frames {
            if frame.dims() != dims {
                return Err(ImageProcError::Processing(format!(
                    "in-memory sequence mixes {}x{}x{} with {}x{}x{}",
                    dims.width,
                    dims.height,
                    dims.channels,
                    frame.width(),
                    frame.height(),
                    frame.channels()
                )));
            }
        }
        Ok(Self { frames, dims })
    }
}

impl BlockSource for LoadedFrames<'_> {
    fn frame_count(&self) -> usize {
        self.frames.len()
    }

    fn dims(&self) -> ImageDims {
        self.dims
    }

    fn read_block_into(
        &self,
        index: usize,
        block: &Block,
        out: &mut [f32],
    ) -> Result<(), ImageProcError> {
        let frame = self.frames.get(index).ok_or_else(|| {
            ImageProcError::Processing(format!(
                "frame index {index} out of range for a {}-frame sequence",
                self.frames.len()
            ))
        })?;
        let rows = frame.rows(block.channel, block.start_row, block.height());
        if out.len() != rows.len() {
            return Err(ImageProcError::Processing(format!(
                "block buffer holds {} samples, need {}",
                out.len(),
                rows.len()
            )));
        }
        out.copy_from_slice(rows);
        Ok(())
    }

    fn first_header(&self) -> Option<px_fits::header::Header> {
        self.frames.first().map(|f| f.header().clone())
    }
}

/// Reduces `set` to a single frame.
///
/// Reads every frame once, in bands sized to fit `runtime`'s budget. The
/// output header is taken from the first frame; callers stamp their own
/// provenance cards when writing.
pub fn stack<S: BlockSource>(
    set: &S,
    runtime: &Runtime,
    options: StackOptions,
) -> Result<StackResult, ImageProcError> {
    stack_calibrated(set, runtime, options, None)
}

/// Reduces `set` to a single frame, subtracting `subtract` from every input
/// frame on the way.
///
/// This is how a master flat is built: the flats must be bias-corrected before
/// they are combined, and doing it per block means the calibrated frames never
/// touch the disk. The subtracted master stays resident for the whole run, so
/// its size is charged against the budget before blocks are planned.
pub fn stack_calibrated<S: BlockSource>(
    set: &S,
    runtime: &Runtime,
    options: StackOptions,
    subtract: Option<&Frame>,
) -> Result<StackResult, ImageProcError> {
    stack_normalized(set, runtime, options, subtract, None)
}

/// [`stack_calibrated`] with per-frame photometric normalization applied as the
/// blocks are read (ADR 007 P11-T2).
///
/// This is the master-light path: frames of one session differ in sky brightness
/// and transparency, and rejection run on unnormalized frames fires on the
/// *frame* rather than on the outlier. Coefficients come from
/// [`normalize::compute`] and are applied per frame and channel, after any
/// `subtract` and before the reduction.
pub fn stack_normalized<S: BlockSource>(
    set: &S,
    runtime: &Runtime,
    options: StackOptions,
    subtract: Option<&Frame>,
    normalization: Option<&Normalization>,
) -> Result<StackResult, ImageProcError> {
    if set.frame_count() == 0 {
        return Err(ImageProcError::EmptySequence);
    }
    if let Method::Mean(r) = options.method {
        r.check_supported()?;
    }

    let dims = set.dims();
    let frames = set.frame_count();
    let elem_size = std::mem::size_of::<f32>();

    if let Some(master) = subtract
        && master.dims() != dims
    {
        return Err(ImageProcError::Processing(format!(
            "frame to subtract is {}x{}x{} but the sequence is {}x{}x{}",
            master.width(),
            master.height(),
            master.channels(),
            dims.width,
            dims.height,
            dims.channels
        )));
    }
    let resident = subtract.map_or(0, |m| m.data().len() as u64 * elem_size as u64);

    // Validated before being simplified away: a caller whose coefficient count
    // does not match the sequence has a bug whether or not those coefficients
    // happen to be identities, and silently ignoring it would mean an
    // unnormalized stack that looks like a normalized one.
    if let Some(n) = normalization
        && n.coefficients.len() != frames
    {
        return Err(ImageProcError::Processing(format!(
            "normalization has {} frames but the sequence has {frames}",
            n.coefficients.len()
        )));
    }
    // All-identity coefficients are dropped rather than carried through the
    // inner loop.
    let normalization = normalization.filter(|n| !n.is_identity());

    let plan = runtime.plan_blocks(
        &dims,
        frames,
        elem_size,
        options.rejection_map_count(),
        resident,
    )?;
    assert_blocks_are_contiguous(&plan.blocks, &dims);

    let mut output = vec![0.0f32; dims.pixel_count()];
    let mut map_low = options
        .rejection_maps
        .then(|| vec![0u16; dims.pixel_count()]);
    let mut map_high = options
        .rejection_maps
        .then(|| vec![0u16; dims.pixel_count()]);

    // One buffer set per worker, sized for the tallest block and reused for
    // every band that worker handles. Allocating per block instead would
    // churn tens of megabytes thousands of times.
    let cores = runtime.cores();
    let buffer_pixels = plan.largest_block_height * dims.width;
    let mut workers: Vec<Worker> = (0..cores)
        .map(|_| {
            let worker = Worker::new(frames, buffer_pixels, options.method);
            if options.rejection_maps {
                worker.with_rejection_maps(buffer_pixels)
            } else {
                worker
            }
        })
        .collect();

    let mut rejected = Rejected::default();
    let mut consumed = 0usize;

    runtime.install(|| -> Result<(), ImageProcError> {
        // Waves of `cores` blocks. Blocks within a wave are consecutive in the
        // plan and therefore contiguous in the output, which is what lets the
        // output be split into disjoint mutable slices without copying.
        for wave in plan.blocks.chunks(cores) {
            let sizes: Vec<usize> = wave.iter().map(|b| b.pixel_count(&dims)).collect();
            let total: usize = sizes.iter().sum();

            let out_slice = &mut output[consumed..consumed + total];
            let mut out_parts = split_sequential_mut(out_slice, &sizes);

            let mut low_parts = map_low
                .as_mut()
                .map(|m| split_sequential_mut(&mut m[consumed..consumed + total], &sizes));
            let mut high_parts = map_high
                .as_mut()
                .map(|m| split_sequential_mut(&mut m[consumed..consumed + total], &sizes));

            let tallies: Vec<Result<Rejected, ImageProcError>> = wave
                .par_iter()
                .zip(workers.par_iter_mut())
                .zip(out_parts.par_iter_mut())
                .map(|((block, worker), out)| {
                    worker.run(set, block, &dims, options, subtract, normalization, out)
                })
                .collect();

            // Each worker holds its block's rejection counts; they are copied
            // out here rather than inside the parallel section, which keeps
            // the closure borrowing nothing but its own worker and slice.
            for (i, tally) in tallies.into_iter().enumerate() {
                let tally = tally?;
                rejected.low += tally.low;
                rejected.high += tally.high;
                if let (Some(low), Some(high)) = (low_parts.as_mut(), high_parts.as_mut()) {
                    low[i].copy_from_slice(&workers[i].map_low[..sizes[i]]);
                    high[i].copy_from_slice(&workers[i].map_high[..sizes[i]]);
                }
            }

            consumed += total;
        }
        Ok(())
    })?;

    debug_assert_eq!(consumed, dims.pixel_count());

    if options.output_norm {
        normalize_to_unit_range(&mut output);
    }

    let header = set.first_header().expect("non-empty set has a first frame");
    let frame = Frame::new(dims, output, header)?;

    Ok(StackResult {
        frame,
        rejected,
        rejection_map_low: map_low,
        rejection_map_high: map_high,
        block_count: plan.blocks.len(),
    })
}

/// Summarizes rather than dumping the maps, which are image-sized.
impl std::fmt::Debug for StackResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StackResult")
            .field("frame", &self.frame)
            .field("rejected", &self.rejected)
            .field("rejection_maps", &self.rejection_map_low.is_some())
            .field("block_count", &self.block_count)
            .finish()
    }
}

/// Per-worker state: the band read from every frame, plus the scratch the
/// reducers need.
struct Worker {
    /// `frames × block_pixels`, frame-major.
    band: Vec<f32>,
    /// One pixel's values across the sequence.
    pixel_stack: Vec<f32>,
    rejection: Option<rejection::Scratch>,
    map_low: Vec<u16>,
    map_high: Vec<u16>,
}

impl Worker {
    fn new(frames: usize, block_pixels: usize, method: Method) -> Self {
        Self {
            band: vec![0.0; frames * block_pixels],
            pixel_stack: vec![0.0; frames],
            rejection: method
                .needs_rejection_scratch()
                .then(|| rejection::Scratch::new(frames)),
            map_low: Vec::new(),
            map_high: Vec::new(),
        }
    }

    /// Allocates the per-block rejection-count buffers. Only called when the
    /// caller asked for the maps, so a run that does not want them does not
    /// carry two more buffers per worker.
    fn with_rejection_maps(mut self, block_pixels: usize) -> Self {
        self.map_low = vec![0; block_pixels];
        self.map_high = vec![0; block_pixels];
        self
    }

    #[allow(clippy::too_many_arguments)]
    fn run<S: BlockSource>(
        &mut self,
        set: &S,
        block: &Block,
        dims: &ImageDims,
        options: StackOptions,
        subtract: Option<&Frame>,
        normalization: Option<&Normalization>,
        out: &mut [f32],
    ) -> Result<Rejected, ImageProcError> {
        let frames = set.frame_count();
        let pixels = block.pixel_count(dims);

        // The master's rows for this block, borrowed once rather than per
        // frame.
        let correction = subtract.map(|m| m.rows(block.channel, block.start_row, block.height()));

        for frame in 0..frames {
            let slot = &mut self.band[frame * pixels..(frame + 1) * pixels];
            set.read_block_into(frame, block, slot)?;
            if let Some(correction) = correction {
                for (v, c) in slot.iter_mut().zip(correction) {
                    *v -= *c;
                }
            }
            // Normalization is per channel, and a block never straddles one
            // (asserted by `assert_blocks_are_contiguous`), so the coefficients
            // are fetched once per frame rather than per pixel.
            if let Some(normalization) = normalization {
                let coefficients = normalization.get(frame, block.channel);
                if !coefficients.is_identity() {
                    for v in slot.iter_mut() {
                        *v = coefficients.apply(*v);
                    }
                }
            }
        }

        let mut tally = Rejected::default();
        for (p, slot) in out.iter_mut().enumerate().take(pixels) {
            for frame in 0..frames {
                self.pixel_stack[frame] = self.band[frame * pixels + p];
            }

            let (value, rejected_here) = reduce(
                &mut self.pixel_stack,
                options.method,
                self.rejection.as_mut(),
            );

            tally.low += rejected_here.low;
            tally.high += rejected_here.high;
            if options.rejection_maps {
                self.map_low[p] = rejected_here.low.min(u16::MAX as u32) as u16;
                self.map_high[p] = rejected_here.high.min(u16::MAX as u32) as u16;
            }

            *slot = if options.output_norm {
                value as f32
            } else {
                (value as f32).clamp(0.0, 1.0)
            };
        }

        Ok(tally)
    }
}

/// Collapses one pixel's values to a single result.
///
/// `stack` is scratch and is reordered freely.
fn reduce(
    stack: &mut [f32],
    method: Method,
    scratch: Option<&mut rejection::Scratch>,
) -> (f64, Rejected) {
    let n = stack.len();
    if n == 0 {
        return (0.0, Rejected::default());
    }

    match method {
        Method::Sum => {
            let mut sum = 0.0f64;
            for &v in stack.iter() {
                sum += v as f64;
            }
            (sum, Rejected::default())
        }
        Method::Min => (
            stack.iter().copied().fold(f32::INFINITY, f32::min) as f64,
            Rejected::default(),
        ),
        Method::Max => (
            stack.iter().copied().fold(f32::NEG_INFINITY, f32::max) as f64,
            Rejected::default(),
        ),
        // Deliberately over every frame, nulls included. A median stack is
        // used for calibration masters, where the frames all cover the same
        // sensor area and a zero is a real reading, not a gap.
        Method::Median => (stats::median_in_place(stack) as f64, Rejected::default()),
        Method::Mean(rule) => {
            let mut counts = Rejected::default();
            let kept = rejection::compact_non_null(stack);
            if kept == 0 {
                return (0.0, counts);
            }
            if kept <= 2 {
                return (mean_of(&stack[..kept]), counts);
            }

            let surviving = match scratch {
                Some(s) => rejection::reject(stack, kept, rule, s, &mut counts),
                None => kept,
            };

            if surviving == 0 {
                // Rejection threw everything away — which means the values
                // disagreed so badly that no centre was credible. The median
                // of what was there beats returning zero.
                (stats::median_in_place(&mut stack[..kept]) as f64, counts)
            } else {
                (mean_of(&stack[..surviving]), counts)
            }
        }
    }
}

fn mean_of(values: &[f32]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sum = 0.0f64;
    for &v in values {
        sum += v as f64;
    }
    sum / values.len() as f64
}

/// Rescales non-zero values to span [0, 1], leaving exact zeros alone.
///
/// Zeros mark absent data, so stretching them into the range would turn gaps
/// into the darkest real signal.
fn normalize_to_unit_range(data: &mut [f32]) {
    let (mut min, mut max) = (f32::INFINITY, f32::NEG_INFINITY);
    for &v in data.iter() {
        if v != 0.0 && !v.is_nan() {
            min = min.min(v);
            max = max.max(v);
        }
    }
    if !min.is_finite() || !max.is_finite() || max <= min {
        return;
    }
    let span = max - min;
    data.par_iter_mut().for_each(|v| {
        if *v != 0.0 {
            *v = (*v - min) / span;
        }
    });
}

/// Splits `slice` into consecutive disjoint pieces of the given sizes.
fn split_sequential_mut<'a, T>(mut slice: &'a mut [T], sizes: &[usize]) -> Vec<&'a mut [T]> {
    let mut parts = Vec::with_capacity(sizes.len());
    for &n in sizes {
        let (head, tail) = slice.split_at_mut(n);
        parts.push(head);
        slice = tail;
    }
    parts
}

/// The wave scheme above depends on plan order matching output order. If the
/// planner ever stopped emitting blocks in channel-then-row order, the stack
/// would silently scramble rows, so it is checked rather than assumed.
fn assert_blocks_are_contiguous(blocks: &[Block], dims: &ImageDims) {
    let mut expected_channel = 0usize;
    let mut expected_row = 0usize;
    for block in blocks {
        assert_eq!(
            (block.channel, block.start_row),
            (expected_channel, expected_row),
            "block plan is not in output order"
        );
        if block.end_row + 1 == dims.height {
            expected_channel += 1;
            expected_row = 0;
        } else {
            expected_row = block.end_row + 1;
        }
    }
}
