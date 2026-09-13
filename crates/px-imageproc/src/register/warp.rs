//! Inverse-mapped warping (ADR 007, P10-T1 / P10-T2 / P10-T3).
//!
//! Warping is done by inverse mapping: for each *output* pixel, the transform's
//! inverse says where in the input it came from, and the kernel interpolates the
//! input there. The forward direction would leave holes wherever the mapping
//! stretches, which is why nobody does it that way.
//!
//! Four kernels, in increasing order of cost and sharpness: bilinear, area,
//! bicubic (the Catmull-Rom-like kernel with `a = -0.75`, as OpenCV's
//! `INTER_CUBIC` uses), and Lanczos-4 (a sinc windowed by a wider sinc over an
//! 8×8 support — Duchon, *Lanczos Filtering in One and Two Dimensions*, J. Appl.
//! Meteor. 18, 1979). Lanczos-4 is the default because it is Siril's, and
//! because at the sampling astrophotography works at the difference between it
//! and bicubic is visible on star cores.
//!
//! Two properties are load-bearing and easy to get wrong:
//!
//! **Pixels with no source are left alone, not filled.** The destination starts
//! at zero and a pixel whose source falls outside the input keeps that zero,
//! which is what the statistics module reads as "no data" (`stats.rs`: zero is
//! null). Filling it with an edge-clamped sample would put invented signal into
//! the stack's rejection and normalization.
//!
//! **Negative lobes need clamping.** Every kernel above bilinear has negative
//! weights, which undershoot at a sharp edge — and a star core is the sharpest
//! edge an astronomical image has. The undershoot shows as a dark ring; the
//! clamping pass below is what removes it.

use rayon::prelude::*;

use super::transform::{Point, Transform};
use crate::error::ImageProcError;
use crate::frame::Frame;
use crate::runtime::{ImageDims, Runtime};

/// Which interpolation kernel a warp uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Interpolation {
    /// 2×2 support, no negative lobes. Fast, and soft.
    Bilinear,
    /// Box average over the source footprint. Appropriate when downscaling,
    /// where a point-sampling kernel aliases; also the guide for clamping.
    Area,
    /// 4×4 support, OpenCV's `INTER_CUBIC` kernel with `a = -0.75`.
    Bicubic,
    /// 8×8 windowed sinc. The default, and Siril's.
    #[default]
    Lanczos4,
}

impl Interpolation {
    /// Half-width of the kernel's support, in source pixels.
    fn radius(&self) -> usize {
        match self {
            Interpolation::Bilinear | Interpolation::Area => 1,
            Interpolation::Bicubic => 2,
            Interpolation::Lanczos4 => 4,
        }
    }

    /// Whether the kernel has negative weights, and so can undershoot.
    pub fn has_negative_lobes(&self) -> bool {
        matches!(self, Interpolation::Bicubic | Interpolation::Lanczos4)
    }
}

/// How a warp is performed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WarpOptions {
    pub interpolation: Interpolation,
    /// Suppress interpolation undershoot around bright cores. On by default, as
    /// in Siril.
    pub clamp: bool,
}

impl Default for WarpOptions {
    fn default() -> Self {
        Self {
            interpolation: Interpolation::default(),
            clamp: true,
        }
    }
}

/// Fraction of the area-interpolated guide below which a pixel is taken to have
/// undershot.
///
/// Siril's `CLAMPING_FACTOR`. A 2% dip is larger than any difference between two
/// reasonable kernels on smooth background and far smaller than the ringing a
/// sinc leaves beside a saturated star, so it separates the two cleanly.
const CLAMPING_FACTOR: f32 = 0.98;

/// Warps `source` into a new frame of `dims` under `transform`.
///
/// `transform` maps *source* pixels onto the output grid — the direction
/// registration produces — and is inverted internally.
pub fn warp(
    source: &Frame,
    transform: &Transform,
    dims: ImageDims,
    runtime: &Runtime,
    options: &WarpOptions,
) -> Result<Frame, ImageProcError> {
    if dims.channels != source.channels() {
        return Err(ImageProcError::Processing(format!(
            "warp output has {} channels but the source has {}",
            dims.channels,
            source.channels()
        )));
    }
    let inverse = transform.invert().ok_or_else(|| {
        ImageProcError::Registration("cannot warp under a singular transform".into())
    })?;

    let mut data = vec![0.0f32; dims.pixel_count()];
    warp_into(
        source,
        &inverse,
        dims,
        runtime,
        options.interpolation,
        &mut data,
    );

    if options.clamp && options.interpolation.has_negative_lobes() {
        // The guide is the same warp with a kernel that cannot undershoot.
        let mut guide = vec![0.0f32; dims.pixel_count()];
        warp_into(
            source,
            &inverse,
            dims,
            runtime,
            Interpolation::Area,
            &mut guide,
        );
        apply_clamp(&mut data, &guide, dims, runtime);
    }

    Frame::new(dims, data, source.header().clone())
}

/// The warp loop, parallel over output row bands.
///
/// Bands rather than rows: each band is an independent slice of the output, and
/// the source rows a band needs are a contiguous run, so a band is also the unit
/// a memory-bounded reader would fetch (P10-T3). The source is already resident
/// here, so the budget's role is to size the bands rather than to gate them.
fn warp_into(
    source: &Frame,
    inverse: &Transform,
    dims: ImageDims,
    runtime: &Runtime,
    interpolation: Interpolation,
    out: &mut [f32],
) {
    let rows_per_band = dims.height.div_ceil(runtime.cores().max(1)).max(1);
    let row_stride = dims.width;
    let plane = dims.width * dims.height;

    runtime.install(|| {
        out.par_chunks_mut(rows_per_band * row_stride)
            .enumerate()
            .for_each(|(band, chunk)| {
                let first_row = band * rows_per_band;
                for (offset, row) in chunk.chunks_mut(row_stride).enumerate() {
                    let y = first_row + offset;
                    // The band index and the channel are both folded into the
                    // flat output index, so a band that straddles a channel
                    // boundary still writes the right plane.
                    for (x, slot) in row.iter_mut().enumerate() {
                        let channel = (y * row_stride + x) / plane;
                        let local_y = y - channel * dims.height;
                        let p = inverse.apply(Point::new(x as f64, local_y as f64));
                        if let Some(value) = sample(source, channel, p, interpolation) {
                            *slot = value;
                        }
                    }
                }
            });
    });
}

/// Interpolates `source` at `p`, or `None` when the kernel's support is not
/// fully inside the frame.
///
/// Requiring the *whole* support, rather than just the centre, is what keeps the
/// edge honest: a Lanczos-4 sample taken two pixels from the border would be
/// computed from four invented rows.
fn sample(source: &Frame, channel: usize, p: Point, interpolation: Interpolation) -> Option<f32> {
    if !p.x.is_finite() || !p.y.is_finite() {
        return None;
    }
    // The kernel reads `floor(p) - (radius - 1) ..= floor(p) + radius` on each
    // axis, so `floor(p)` must lie in `[radius - 1, extent - 1 - radius]` —
    // which, in terms of `p` itself, is a closed lower bound and a *half-open*
    // upper one. Getting that boundary wrong reads one row past the frame.
    let (width, height) = (source.width() as f64, source.height() as f64);
    let radius = interpolation.radius() as f64;
    if p.x < radius - 1.0 || p.y < radius - 1.0 || p.x >= width - radius || p.y >= height - radius {
        return None;
    }

    Some(match interpolation {
        Interpolation::Bilinear => bilinear(source, channel, p),
        Interpolation::Area => area(source, channel, p),
        Interpolation::Bicubic => separable(source, channel, p, 2, bicubic_table()),
        Interpolation::Lanczos4 => separable(source, channel, p, 4, lanczos4_table()),
    })
}

fn bilinear(source: &Frame, channel: usize, p: Point) -> f32 {
    let x0 = p.x.floor();
    let y0 = p.y.floor();
    let (fx, fy) = ((p.x - x0) as f32, (p.y - y0) as f32);
    let (x0, y0) = (x0 as usize, y0 as usize);

    let v00 = source.get(channel, x0, y0);
    let v10 = source.get(channel, x0 + 1, y0);
    let v01 = source.get(channel, x0, y0 + 1);
    let v11 = source.get(channel, x0 + 1, y0 + 1);
    let top = v00 + (v10 - v00) * fx;
    let bottom = v01 + (v11 - v01) * fx;
    top + (bottom - top) * fy
}

/// Average of the source over the output pixel's footprint.
///
/// Source pixel `i` covers `[i − ½, i + ½]` and the output pixel's footprint is
/// the unit box centred on `p`, so the overlap weights work out to `(1 − f, f)`
/// per axis — which is bilinear. That is not a shortcut: at the unit scale
/// registration works at, the footprint average *is* bilinear, and the two
/// variants differ only in intent.
///
/// A genuinely downscaling warp, where one output pixel covers several source
/// pixels, would need the footprint from the transform's local Jacobian. That is
/// out of scope here (ADR 007 Scope excludes drizzle and rescaling), and this
/// kernel's job in the warp is to be the clamping guide: an interpolation that
/// cannot undershoot, sampled at exactly the same point as the real one.
///
/// Getting the centring wrong is not a subtle cosmetic matter. Averaging the
/// 2×2 block starting at `floor(p)` — the obvious reading of "box filter" —
/// shifts the guide half a pixel against the warp it is guiding, and the
/// clamping mask then flags 11% of a frame instead of the 1% that actually rang.
fn area(source: &Frame, channel: usize, p: Point) -> f32 {
    bilinear(source, channel, p)
}

/// Sub-pixel positions the weight tables are computed at.
///
/// A Lanczos-4 sample costs 32 `sin` evaluations if the weights are computed per
/// pixel, which dominates the warp — measured at 182 ms for 16.8 MP against 7 ms
/// for bilinear. The weights depend only on the *fractional* part of the
/// position, so they are precomputed on a fixed grid and looked up, which is what
/// OpenCV's `INTER_BITS` does (it uses 32 steps; this uses 1024, so the
/// quantization is 32× finer).
///
/// The cost of quantizing is that a sample is taken at a position rounded to
/// 1/1024 px. At the steepest gradient an astronomical image has — the flank of
/// a critically sampled star, where intensity changes by perhaps 30% of the peak
/// per pixel — that is a 3e-4 relative error, two orders of magnitude below the
/// f32 noise floor of the data and four below the Phase 10 parity gate.
const WEIGHT_STEPS: usize = 1024;

/// Precomputed, normalized kernel weights: `WEIGHT_STEPS` rows of `taps`.
struct WeightTable {
    taps: usize,
    weights: Vec<f64>,
}

impl WeightTable {
    fn build(radius: usize, weight: fn(f64) -> f64) -> Self {
        let taps = 2 * radius;
        let mut weights = Vec::with_capacity(WEIGHT_STEPS * taps);
        for step in 0..WEIGHT_STEPS {
            let fraction = step as f64 / WEIGHT_STEPS as f64;
            let row: Vec<f64> = (0..taps)
                .map(|i| weight(i as f64 - (radius as f64 - 1.0) - fraction))
                .collect();
            // Normalized here, once, rather than per sample: the analytic
            // kernels sum to 1 only in the limit, and an unnormalized sum shows
            // up as a few-percent brightness ripple across the frame.
            let sum: f64 = row.iter().sum();
            let scale = if sum == 0.0 { 0.0 } else { 1.0 / sum };
            weights.extend(row.into_iter().map(|w| w * scale));
        }
        Self { taps, weights }
    }

    #[inline]
    fn row(&self, fraction: f64) -> &[f64] {
        // `min` rather than a modulo: a fraction of exactly 1.0 can arrive from
        // rounding and must land on the last row, not wrap to the first.
        let step = ((fraction * WEIGHT_STEPS as f64) as usize).min(WEIGHT_STEPS - 1);
        &self.weights[step * self.taps..(step + 1) * self.taps]
    }
}

fn bicubic_table() -> &'static WeightTable {
    static TABLE: std::sync::OnceLock<WeightTable> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| WeightTable::build(2, cubic_weight))
}

fn lanczos4_table() -> &'static WeightTable {
    static TABLE: std::sync::OnceLock<WeightTable> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| WeightTable::build(4, lanczos4_weight))
}

/// A separable kernel of half-width `radius`, evaluated as the outer product of
/// its row and column weights.
fn separable(source: &Frame, channel: usize, p: Point, radius: usize, table: &WeightTable) -> f32 {
    let x0 = p.x.floor();
    let y0 = p.y.floor();
    let wx = table.row(p.x - x0);
    let wy = table.row(p.y - y0);
    let (x0, y0) = (x0 as isize, y0 as isize);

    let mut total = 0.0f64;
    for (j, &weight_y) in wy.iter().enumerate() {
        let y = y0 + j as isize - (radius as isize - 1);
        let mut row = 0.0f64;
        for (i, &weight_x) in wx.iter().enumerate() {
            let x = x0 + i as isize - (radius as isize - 1);
            row += weight_x * source.get(channel, x as usize, y as usize) as f64;
        }
        total += weight_y * row;
    }
    total as f32
}

/// OpenCV's `INTER_CUBIC` kernel: the two-piece cubic with `a = -0.75`.
fn cubic_weight(t: f64) -> f64 {
    const A: f64 = -0.75;
    let t = t.abs();
    if t <= 1.0 {
        ((A + 2.0) * t - (A + 3.0)) * t * t + 1.0
    } else if t < 2.0 {
        ((A * t - 5.0 * A) * t + 8.0 * A) * t - 4.0 * A
    } else {
        0.0
    }
}

/// Lanczos-4: `sinc(t) · sinc(t/4)` over `|t| < 4`.
fn lanczos4_weight(t: f64) -> f64 {
    const SUPPORT: f64 = 4.0;
    if t == 0.0 {
        return 1.0;
    }
    if t.abs() >= SUPPORT {
        return 0.0;
    }
    let pt = std::f64::consts::PI * t;
    SUPPORT * pt.sin() * (pt / SUPPORT).sin() / (pt * pt)
}

/// Replaces undershot pixels with the guide's value (ADR 007 P10-T2).
///
/// Three steps, and the middle one is the reason this is not just a per-pixel
/// `max`: mark the pixels that fell below `CLAMPING_FACTOR` of the guide, dilate
/// that mask by one pixel, then copy the guide in. The dilation matters because
/// ringing is a *ring* — the deepest undershoot is flanked by pixels that dipped
/// less than 2% and would otherwise be left as a faint residual edge around a
/// repaired core.
/// A fourth step is needed that is easy to miss: the repair must not write a
/// pixel the warp itself left empty. The guide's kernel has a 2×2 support where
/// Lanczos-4 has 8×8, so in a four-pixel band around the footprint's edge the
/// guide has a value and the real warp does not — and filling those from the
/// guide would invent an eight-pixel frame of data the output is supposed to
/// leave unexposed, in a band the stacker would then treat as real signal.
fn apply_clamp(data: &mut [f32], guide: &[f32], dims: ImageDims, runtime: &Runtime) {
    let mask = undershoot_mask(data, guide);
    let dilated = dilate3x3(&mask, dims, runtime);
    for (i, flagged) in dilated.iter().enumerate() {
        if *flagged && data[i] != 0.0 {
            data[i] = guide[i];
        }
    }
}

fn undershoot_mask(data: &[f32], guide: &[f32]) -> Vec<bool> {
    data.iter()
        .zip(guide)
        .map(|(&value, &guide)| value != 0.0 && guide > 0.0 && value < guide * CLAMPING_FACTOR)
        .collect()
}

/// Dilates a mask with a 3×3 elliptical structuring element — which on a 3×3
/// grid is the plus-shaped one, since an ellipse inscribed in a 3×3 box does not
/// reach its corners.
///
/// Channels are dilated independently: a mask that leaked across a channel
/// boundary would repair the top row of green from the bottom row of red.
pub fn dilate3x3(mask: &[bool], dims: ImageDims, runtime: &Runtime) -> Vec<bool> {
    let (width, height) = (dims.width, dims.height);
    let plane = width * height;
    let mut out = vec![false; mask.len()];

    runtime.install(|| {
        out.par_chunks_mut(width)
            .enumerate()
            .for_each(|(flat_row, row)| {
                let channel = flat_row / height;
                let y = flat_row % height;
                let base = channel * plane;
                for (x, slot) in row.iter_mut().enumerate() {
                    let at = |dx: isize, dy: isize| -> bool {
                        let nx = x as isize + dx;
                        let ny = y as isize + dy;
                        if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                            return false;
                        }
                        mask[base + ny as usize * width + nx as usize]
                    };
                    *slot = at(0, 0) || at(-1, 0) || at(1, 0) || at(0, -1) || at(0, 1);
                }
            });
    });
    out
}
