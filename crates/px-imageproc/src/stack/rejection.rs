//! Outlier rejection for mean stacking (ADR 007, P3-T5).
//!
//! Every routine here works on one pixel's stack — the `n` values that pixel
//! takes across the sequence — and partitions it into kept and rejected. They
//! run once per output pixel, so a 61 MP stack calls them 61 million times;
//! that is why they take a caller-owned scratch buffer and never allocate.
//!
//! The shared shape is: estimate a centre and a spread, drop whatever lies
//! more than `low`/`high` multiples of the spread away, re-estimate, repeat
//! until nothing more is dropped. What varies is the spread estimator, which
//! is what makes each method suited to a different frame count — a standard
//! deviation is fine with many frames and useless with six, where it is
//! dominated by the outlier it is supposed to find.

use crate::error::ImageProcError;
use crate::stats;

/// Rejection bounds, in units of the method's spread estimate. Asymmetric
/// because cosmic rays and satellites are bright: the high side is usually
/// tightened relative to the low.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub low: f32,
    pub high: f32,
}

impl Bounds {
    pub fn new(low: f32, high: f32) -> Self {
        Self { low, high }
    }

    pub fn symmetric(sigma: f32) -> Self {
        Self {
            low: sigma,
            high: sigma,
        }
    }
}

/// How outliers are identified before the surviving values are averaged.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rejection {
    /// Average everything (after null removal).
    None,
    /// Bounds are fractions *of the median value itself*, not of a spread.
    /// The only method that works with very few frames, where no spread
    /// estimate is trustworthy.
    Percentile(Bounds),
    /// Iterated clipping about the median, spread from the standard
    /// deviation.
    Sigma(Bounds),
    /// Iterated clipping about the median, spread from the MAD — more robust
    /// than [`Rejection::Sigma`] when outliers are numerous.
    Mad(Bounds),
    /// Like [`Rejection::Sigma`], but outliers are *replaced* by the median
    /// rather than dropped, so every frame keeps contributing.
    SigmaMedian(Bounds),
    /// Iterated clipping whose spread comes from a Winsorized standard
    /// deviation: the tails are pulled in to ±1.5σ before measuring, so the
    /// outliers cannot inflate the very estimate meant to catch them.
    Winsorized(Bounds),
    /// Fits a line to the sorted stack and clips on the residual. Suits long
    /// sequences with a gradual trend — sky brightening, a drifting gradient
    /// — that a constant centre would mistake for spread.
    LinearFit(Bounds),
    /// Generalized Extreme Studentized Deviate. **Not implemented**: it needs
    /// the inverse Student's t CDF, and nothing in the project selects it.
    /// Requesting it is a typed error rather than a silent fallback.
    Gesdt(Bounds),
}

impl Rejection {
    pub fn bounds(&self) -> Option<Bounds> {
        match self {
            Rejection::None => None,
            Rejection::Percentile(b)
            | Rejection::Sigma(b)
            | Rejection::Mad(b)
            | Rejection::SigmaMedian(b)
            | Rejection::Winsorized(b)
            | Rejection::LinearFit(b)
            | Rejection::Gesdt(b) => Some(*b),
        }
    }

    /// Rejects up front what cannot be run, so a stack fails before reading
    /// frames rather than partway through.
    pub fn check_supported(&self) -> Result<(), ImageProcError> {
        match self {
            Rejection::Gesdt(_) => Err(ImageProcError::Unsupported(
                "GESDT rejection is not implemented; use Winsorized or LinearFit".to_string(),
            )),
            _ => Ok(()),
        }
    }
}

/// How many values were dropped from each tail.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rejected {
    pub low: u32,
    pub high: u32,
}

impl Rejected {
    pub fn total(&self) -> u32 {
        self.low + self.high
    }
}

/// Below this many surviving values, clipping stops. Rejecting down to a
/// handful makes the remaining estimate meaningless, and a pixel where every
/// frame disagrees is better served by keeping them all.
const MIN_KEPT: usize = 4;

/// Scratch the rejection routines need, sized once per thread rather than per
/// pixel.
pub struct Scratch {
    /// Winsorized copy of the stack.
    winsorized: Vec<f32>,
    /// Per-value verdict: -1 low, 0 keep, +1 high.
    verdict: Vec<i8>,
}

impl Scratch {
    pub fn new(frames: usize) -> Self {
        Self {
            winsorized: vec![0.0; frames],
            verdict: vec![0; frames],
        }
    }
}

/// Drops null values from `stack`, compacting survivors to the front and
/// returning how many there are.
///
/// A zero means the pixel had no data in that frame — outside a warped
/// frame's footprint, or an unexposed region — so it must not pull the
/// average toward zero.
pub fn compact_non_null(stack: &mut [f32]) -> usize {
    let mut kept = 0;
    for i in 0..stack.len() {
        if stack[i] != 0.0 && !stack[i].is_nan() {
            stack[kept] = stack[i];
            kept += 1;
        }
    }
    kept
}

/// Applies `rejection` to `stack[..n]`, compacting the survivors to the front
/// and returning how many survived.
///
/// `stack` is reordered freely. Returns 0 only when the stack is degenerate
/// enough that the caller should fall back to a median.
pub fn reject(
    stack: &mut [f32],
    n: usize,
    rejection: Rejection,
    scratch: &mut Scratch,
    counts: &mut Rejected,
) -> usize {
    // One or two values leave nothing to reject against.
    if n <= 2 || matches!(rejection, Rejection::None) {
        return n;
    }

    match rejection {
        Rejection::None | Rejection::Gesdt(_) => n,
        Rejection::Percentile(b) => percentile(stack, n, b, scratch, counts),
        Rejection::Sigma(b) => iterated_clip(stack, n, b, scratch, counts, Spread::StdDev),
        Rejection::Mad(b) => iterated_clip(stack, n, b, scratch, counts, Spread::Mad),
        Rejection::Winsorized(b) => iterated_clip(stack, n, b, scratch, counts, Spread::Winsorized),
        Rejection::SigmaMedian(b) => sigma_median(stack, n, b, counts),
        Rejection::LinearFit(b) => linear_fit(stack, n, b, scratch, counts),
    }
}

/// Which spread estimate drives an iterated clip.
#[derive(Clone, Copy, PartialEq)]
enum Spread {
    StdDev,
    Mad,
    Winsorized,
}

/// Sample standard deviation of `data`, and its mean.
fn std_dev(data: &[f32]) -> (f32, f32) {
    let n = data.len();
    if n < 2 {
        return (0.0, data.first().copied().unwrap_or(0.0));
    }
    let mut sum = 0.0f64;
    for &v in data {
        sum += v as f64;
    }
    let mean = (sum / n as f64) as f32;

    let mut acc = 0.0f64;
    for &v in data {
        let d = (v - mean) as f64;
        acc += d * d;
    }
    // Sample form: this is an estimate from n draws, and at the frame counts
    // that matter (6-40) the difference from the population form is not
    // negligible.
    (((acc / (n - 1) as f64) as f32).sqrt(), mean)
}

fn median_of(data: &mut [f32]) -> f32 {
    stats::median_in_place(data)
}

fn mad_about(data: &[f32], centre: f32, scratch: &mut Vec<f32>) -> f32 {
    scratch.clear();
    scratch.extend(data.iter().map(|v| (v - centre).abs()));
    stats::median_in_place(scratch)
}

/// Rejection at fixed fractions of the median value.
///
/// The bounds are relative to the median *level*, not to a spread — with only
/// a handful of frames there is no spread estimate worth trusting, but the
/// median level is still meaningful.
fn percentile(
    stack: &mut [f32],
    n: usize,
    bounds: Bounds,
    scratch: &mut Scratch,
    counts: &mut Rejected,
) -> usize {
    scratch.winsorized[..n].copy_from_slice(&stack[..n]);
    let median = median_of(&mut scratch.winsorized[..n]);
    if median == 0.0 {
        return 0;
    }

    let mut kept = 0;
    for i in 0..n {
        let v = stack[i];
        if median - v > median * bounds.low {
            counts.low += 1;
        } else if v - median > median * bounds.high {
            counts.high += 1;
        } else {
            stack[kept] = v;
            kept += 1;
        }
    }
    kept
}

/// Iterated clipping about the median, with the spread estimate chosen by
/// `spread`. Loops until a pass drops nothing, or the survivors hit the floor.
fn iterated_clip(
    stack: &mut [f32],
    mut n: usize,
    bounds: Bounds,
    scratch: &mut Scratch,
    counts: &mut Rejected,
    spread: Spread,
) -> usize {
    let mut deviations: Vec<f32> = Vec::new();

    scratch.winsorized[..n].copy_from_slice(&stack[..n]);
    let mut median = median_of(&mut scratch.winsorized[..n]);

    // Cumulative across iterations, not per pass: once this many values have
    // gone, the stack is too thin to keep cutting.
    let mut removed = 0usize;
    let mut first_pass = true;

    loop {
        // Winsorizing pulls the tails in toward the centre, so it has to use
        // *this* pass's centre — measuring the pulled-in spread around a
        // centre the last pass left behind would winsorize against the wrong
        // point. The clipping estimators have no such coupling: they measure
        // the spread about the previous centre and then move it, which avoids
        // understating the spread on a pass that just removed something.
        if spread == Spread::Winsorized && !first_pass {
            scratch.winsorized[..n].copy_from_slice(&stack[..n]);
            median = median_of(&mut scratch.winsorized[..n]);
        }

        let sigma = match spread {
            Spread::StdDev => std_dev(&stack[..n]).0,
            Spread::Mad => mad_about(&stack[..n], median, &mut deviations),
            Spread::Winsorized => winsorized_sigma(&stack[..n], median, &mut scratch.winsorized),
        };

        if first_pass {
            first_pass = false;
        } else if spread != Spread::Winsorized {
            scratch.winsorized[..n].copy_from_slice(&stack[..n]);
            median = median_of(&mut scratch.winsorized[..n]);
        }

        if sigma <= 0.0 {
            return n;
        }

        let mut kept = 0;
        for i in 0..n {
            let v = stack[i];
            if n.saturating_sub(removed) <= MIN_KEPT {
                stack[kept] = v;
                kept += 1;
                continue;
            }
            if median - v > sigma * bounds.low {
                counts.low += 1;
                removed += 1;
            } else if v - median > sigma * bounds.high {
                counts.high += 1;
                removed += 1;
            } else {
                stack[kept] = v;
                kept += 1;
            }
        }

        let changed = kept != n;
        n = kept;
        if !changed || n < MIN_KEPT {
            return n;
        }
    }
}

/// Standard deviation of the stack after its tails are pulled in to
/// ±1.5σ about the median, iterated to convergence and rescaled by the
/// factor that makes it consistent with a true sigma for normal data.
fn winsorized_sigma(data: &[f32], median: f32, scratch: &mut [f32]) -> f32 {
    let n = data.len();
    scratch[..n].copy_from_slice(data);

    let mut sigma = std_dev(data).0;
    if sigma <= 0.0 {
        return 0.0;
    }

    loop {
        let lo = median - 1.5 * sigma;
        let hi = median + 1.5 * sigma;
        for v in scratch[..n].iter_mut() {
            *v = v.clamp(lo, hi);
        }
        let previous = sigma;
        sigma = 1.134 * std_dev(&scratch[..n]).0;
        if (sigma - previous).abs() <= previous * 0.0005 {
            return sigma;
        }
        if sigma <= 0.0 {
            return 0.0;
        }
    }
}

/// Replaces outliers with the median instead of dropping them, so the frame
/// count stays constant. Repeats until a pass replaces nothing.
fn sigma_median(stack: &mut [f32], n: usize, bounds: Bounds, counts: &mut Rejected) -> usize {
    let mut copy: Vec<f32> = Vec::with_capacity(n);
    loop {
        let sigma = std_dev(&stack[..n]).0;
        copy.clear();
        copy.extend_from_slice(&stack[..n]);
        let median = median_of(&mut copy);

        if sigma <= 0.0 {
            return n;
        }

        let mut replaced = 0;
        for v in stack[..n].iter_mut() {
            if median - *v > sigma * bounds.low {
                counts.low += 1;
                *v = median;
                replaced += 1;
            } else if *v - median > sigma * bounds.high {
                counts.high += 1;
                *v = median;
                replaced += 1;
            }
        }
        if replaced == 0 {
            return n;
        }
    }
}

/// Fits a line through the sorted stack and clips on the residual.
///
/// **This differs deliberately from the reference implementation.** Siril
/// precomputes the index mean and index variance once for the *full* frame
/// count and keeps using them as the stack shrinks across clipping
/// iterations, so every pass after the first fits against constants that no
/// longer describe its data. Recomputing them for the current length is a
/// two-line change and is what a least-squares fit means; the divergence is
/// measured in `tests/stack_parity.rs` rather than assumed harmless.
fn linear_fit(
    stack: &mut [f32],
    mut n: usize,
    bounds: Bounds,
    scratch: &mut Scratch,
    counts: &mut Rejected,
) -> usize {
    let mut removed = 0usize;
    loop {
        stack[..n].sort_by(f32::total_cmp);

        let (intercept, slope) = least_squares_over_index(&stack[..n]);

        // Spread is the mean absolute residual, not an RMS: the whole point
        // is that the stack still contains the outliers being looked for.
        let mut residual = 0.0f64;
        for (i, &v) in stack[..n].iter().enumerate() {
            residual += (v - (slope * i as f32 + intercept)).abs() as f64;
        }
        let sigma = (residual / n as f64) as f32;
        if sigma <= 0.0 {
            return n;
        }

        for (i, verdict) in scratch.verdict[..n].iter_mut().enumerate() {
            let fitted = slope * i as f32 + intercept;
            let v = stack[i];
            *verdict = if fitted - v > sigma * bounds.low {
                -1
            } else if v - fitted > sigma * bounds.high {
                1
            } else {
                0
            };
        }

        let mut kept = 0;
        for i in 0..n {
            if n.saturating_sub(removed) <= MIN_KEPT {
                stack[kept] = stack[i];
                kept += 1;
                continue;
            }
            match scratch.verdict[i] {
                -1 => {
                    counts.low += 1;
                    removed += 1;
                }
                1 => {
                    counts.high += 1;
                    removed += 1;
                }
                _ => {
                    stack[kept] = stack[i];
                    kept += 1;
                }
            }
        }

        let changed = kept != n;
        n = kept;
        if !changed || n < MIN_KEPT {
            return n;
        }
    }
}

/// Least-squares fit of `y` against its own index, returning
/// `(intercept, slope)`.
fn least_squares_over_index(y: &[f32]) -> (f32, f32) {
    let n = y.len();
    if n < 2 {
        return (y.first().copied().unwrap_or(0.0), 0.0);
    }

    let mean_x = (n - 1) as f64 * 0.5;
    let mut mean_y = 0.0f64;
    for &v in y {
        mean_y += v as f64;
    }
    mean_y /= n as f64;

    let mut sxy = 0.0f64;
    let mut sxx = 0.0f64;
    for (i, &v) in y.iter().enumerate() {
        let dx = i as f64 - mean_x;
        sxy += dx * (v as f64 - mean_y);
        sxx += dx * dx;
    }

    let slope = if sxx == 0.0 { 0.0 } else { sxy / sxx };
    ((mean_y - mean_x * slope) as f32, slope as f32)
}
