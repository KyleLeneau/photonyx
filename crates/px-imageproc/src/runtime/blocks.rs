//! Partitioning an image into memory-bounded bands of rows (ADR 007, D3).
//!
//! Reducing a sequence — a median, a rejected mean — needs every frame's value
//! for a given pixel at the same time, and a night of frames does not fit in
//! memory. The way out is to process a band of rows at a time: read that same
//! band from every frame, reduce it pixel by pixel, write it, move on. Peak
//! memory is then set by the band height rather than by the sequence length.
//!
//! This module chooses that band height. The constraints are:
//!
//! 1. The bands resident at once must fit the memory budget. With `w` workers
//!    each holding one band of `h` rows from all `n` frames, that is
//!    `w × h × width × n × 4` bytes.
//! 2. Every worker should get a comparable amount of work, so none is still
//!    going long after the rest have finished.
//! 3. A band must not straddle a channel. The reduction reads one channel's
//!    rows from each frame, and a band spanning two would not be contiguous in
//!    any of them.
//!
//! Solving (1) for `h` gives the band height directly; (2) and (3) then fall
//! out of how the rows are divided up.

use crate::error::ImageProcError;

/// Rounds `value` up to the next multiple of `factor`.
fn round_up(value: usize, factor: usize) -> usize {
    if factor == 0 {
        return value;
    }
    value.div_ceil(factor) * factor
}

/// Dimensions of the image being produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDims {
    pub width: usize,
    /// Rows per channel.
    pub height: usize,
    /// 1 for mono, 3 for colour.
    pub channels: usize,
}

impl ImageDims {
    pub fn new(width: usize, height: usize, channels: usize) -> Self {
        Self {
            width,
            height,
            channels,
        }
    }

    /// Pixels in the whole image, across all channels.
    pub fn pixel_count(&self) -> usize {
        self.width * self.height * self.channels
    }

    /// Rows across all channels.
    pub fn total_rows(&self) -> usize {
        self.height * self.channels
    }
}

/// One unit of work: a run of rows within a single channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block {
    pub channel: usize,
    /// First row of this block within its channel, inclusive.
    pub start_row: usize,
    /// Last row of this block within its channel, inclusive.
    pub end_row: usize,
}

impl Block {
    pub fn height(&self) -> usize {
        self.end_row - self.start_row + 1
    }

    /// Pixels in this block for one frame.
    pub fn pixel_count(&self, dims: &ImageDims) -> usize {
        self.height() * dims.width
    }
}

/// A complete partition of an image into [`Block`]s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockPlan {
    pub blocks: Vec<Block>,
    /// Height of the tallest block. Each worker's buffer is sized for this, so
    /// one buffer serves any block it is handed.
    pub largest_block_height: usize,
}

/// How many rows of one channel a single worker may hold from every frame at
/// once, given the bytes available for band buffers.
///
/// Returns 0 when the budget cannot hold even one row per worker, which the
/// caller must treat as "cannot proceed" rather than dividing by.
pub fn rows_per_worker(
    bytes_for_bands: u64,
    width: usize,
    nb_frames: usize,
    elem_size: usize,
    workers: usize,
) -> usize {
    let per_row_all_frames = nb_frames as u64 * width as u64 * elem_size as u64;
    if per_row_all_frames == 0 || workers == 0 {
        return 0;
    }

    // Besides its band, each worker holds one pixel's values across the
    // sequence — the scratch the reducers work in. Tiny next to a band, but
    // left out it is exactly the margin by which a plan overruns its budget.
    let working_stack = workers as u64 * nb_frames as u64 * elem_size as u64;
    let for_bands = bytes_for_bands.saturating_sub(working_stack);

    let bytes_per_worker = for_bands / workers as u64;
    (bytes_per_worker / per_row_all_frames) as usize
}

impl BlockPlan {
    /// Partitions `dims` into blocks of at most `max_band_height` rows,
    /// arranged so `workers` share the load.
    ///
    /// Blocks are emitted in output order — channel by channel, top to bottom
    /// within each — so a consumer can write them into a contiguous buffer
    /// without seeking.
    pub fn compute(
        max_band_height: usize,
        dims: &ImageDims,
        workers: usize,
    ) -> Result<Self, ImageProcError> {
        if workers < 1 {
            return Err(ImageProcError::Processing(
                "block planning needs at least one worker".to_string(),
            ));
        }
        if dims.width == 0 || dims.height == 0 || dims.channels == 0 {
            return Err(ImageProcError::Processing(format!(
                "cannot plan blocks for an empty image: {dims:?}"
            )));
        }
        if max_band_height < 1 {
            return Err(ImageProcError::InsufficientMemory {
                what: "one row of every frame, per worker".to_string(),
            });
        }

        // Fewest bands per channel that respect the height cap.
        let by_memory = dims.height.div_ceil(max_band_height.min(dims.height));

        // Workers take bands in waves, so a run costs
        // `ceil(bands / workers)` waves and every wave costs one band's time,
        // however few workers that wave actually occupies. A band count just
        // over a multiple of the worker count is therefore the worst case:
        // 19 bands on 16 cores costs two full waves, the second running three
        // cores while thirteen idle.
        //
        // Rounding the total up to a whole number of waves fixes it for free.
        // The extra bands are shorter, not additional work — the memory cap is
        // an upper bound, so splitting further always satisfies it — and the
        // same two waves then cost two *short* bands instead of two tall ones.
        let target_total = round_up(by_memory * dims.channels, workers).max(workers);
        let bands_per_channel = target_total
            .div_ceil(dims.channels)
            .max(by_memory)
            .min(dims.height);
        let base_height = dims.height / bands_per_channel;

        let mut blocks = Vec::with_capacity(bands_per_channel * dims.channels);
        let mut largest_block_height = 0usize;

        for channel in 0..dims.channels {
            let mut row = 0usize;
            // Rows left over after an even split go one each to the leading
            // bands, so no two bands differ by more than a single row.
            let mut remainder = dims.height % bands_per_channel;

            for _ in 0..bands_per_channel {
                let mut height = base_height;
                if remainder > 0 {
                    height += 1;
                    remainder -= 1;
                }
                let block = Block {
                    channel,
                    start_row: row,
                    end_row: row + height - 1,
                };
                largest_block_height = largest_block_height.max(height);
                blocks.push(block);
                row += height;
            }
            debug_assert_eq!(row, dims.height, "bands must tile the channel exactly");
        }

        Ok(Self {
            blocks,
            largest_block_height,
        })
    }

    /// Bytes one worker's band buffer needs: the tallest block, held for every
    /// frame, plus one frame-sized run used as the per-pixel working stack.
    pub fn buffer_bytes_per_worker(
        &self,
        dims: &ImageDims,
        nb_frames: usize,
        elem_size: usize,
    ) -> u64 {
        let pixels_in_block = self.largest_block_height as u64 * dims.width as u64;
        nb_frames as u64 * (pixels_in_block + 1) * elem_size as u64
    }
}
