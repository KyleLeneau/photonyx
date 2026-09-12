//! Outlier rejection for mean stacking (ADR 007, Phase 3).
//!
//! Every routine here works on one pixel's stack — the `n` values that pixel
//! takes across the sequence — and partitions it into kept and rejected. They
//! run once per output pixel, so a 61 MP stack calls them 61 million times;
//! that is why they take a caller-owned scratch buffer and never allocate.
//!
//! All but one share a shape: estimate a centre and a spread, drop whatever
//! lies more than `low`/`high` multiples of the spread from the centre,
//! re-estimate, and repeat until a pass drops nothing. What varies is the
//! spread estimator, and that is what suits each method to a different frame
//! count — a standard deviation is fine with many frames and useless with six,
//! where the outlier it is meant to find is what dominates it.
//!
//! The estimators are standard:
//!
//! - Sigma and MAD clipping are the textbook iterated k-sigma clip, the second
//!   swapping the standard deviation for the median absolute deviation.
//! - Winsorized sigma clipping follows Huber's proposal: pull the tails in to
//!   ±1.5σ, re-measure, iterate to convergence, and rescale by 1.134 to undo
//!   the bias that pulling them in introduces for normally distributed data.
//! - Percentile clipping thresholds on a fraction of the median *level* rather
//!   than on a spread, which is what makes it usable when there are too few
//!   frames to estimate a spread at all.
//! - Linear-fit clipping fits a least-squares line through the sorted values
//!   and clips on the residual, so a stack with a genuine trend across the
//!   sequence is not mistaken for one with a wide spread.

use crate::error::ImageProcError;
use crate::stats;

/// Rejection bounds, in units of the method's spread estimate. Asymmetric
/// because the outliers that matter are usually bright — cosmic rays,
/// satellites, aircraft — so the high side is typically tightened.
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
    /// Iterated clipping about the median, spread from the standard deviation.
    Sigma(Bounds),
    /// Iterated clipping about the median, spread from the MAD — more robust
    /// than [`Rejection::Sigma`] when outliers are numerous.
    Mad(Bounds),
    /// Like [`Rejection::Sigma`], but outliers are *replaced* by the median
    /// rather than dropped, so every frame keeps contributing.
    SigmaMedian(Bounds),
    /// Iterated clipping whose spread comes from a Winsorized standard
    /// deviation, so the outliers cannot inflate the estimate meant to catch
    /// them.
    Winsorized(Bounds),
    /// Fits a line to the sorted stack and clips on the residual. Suits long
    /// sequences with a gradual trend — sky brightening, a drifting gradient —
    /// that a constant centre would mistake for spread.
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

/// Fewest values a clip may leave behind.
///
/// Below this there is nothing left to estimate a spread from, so further
/// clipping would be measuring its own noise. A pixel where that many frames
/// genuinely disagree is better served by keeping them all and letting the
/// mean say so.
const MIN_KEPT: usize = 4;

/// Winsorizing pulls the tails to this many sigma before re-measuring.
const WINSOR_LIMIT: f32 = 1.5;

/// Rescales a Winsorized standard deviation so it estimates the same quantity
/// as an ordinary one for normally distributed data.
const WINSOR_CONSISTENCY: f32 = 1.134;

/// Relative change below which the Winsorized sigma iteration has converged.
const WINSOR_TOLERANCE: f32 = 5e-4;

/// Scratch the rejection routines need, sized once per worker rather than per
/// pixel.
pub struct Scratch {
    /// Working copy for estimators that must not disturb the stack.
    work: Vec<f32>,
    /// Absolute deviations, for the MAD estimator.
    deviations: Vec<f32>,
}

impl Scratch {
    pub fn new(frames: usize) -> Self {
        Self {
            work: vec![0.0; frames],
            deviations: vec![0.0; frames],
        }
    }
}

/// Drops null values from `stack`, compacting survivors to the front and
/// returning how many there are.
///
/// A zero means the pixel had no data in that frame — outside a warped
/// frame's footprint, or an unexposed region — so it must not pull the average
/// toward zero.
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
/// `stack` is reordered freely.
pub fn reject(
    stack: &mut [f32],
    n: usize,
    rejection: Rejection,
    scratch: &mut Scratch,
    counts: &mut Rejected,
) -> usize {
    // Two values leave nothing to reject against: whichever you drop, the
    // other is the answer.
    if n <= 2 || matches!(rejection, Rejection::None | Rejection::Gesdt(_)) {
        return n;
    }

    match rejection {
        Rejection::None | Rejection::Gesdt(_) => n,
        Rejection::Percentile(b) => percentile(stack, n, b, scratch, counts),
        Rejection::SigmaMedian(b) => sigma_median(stack, n, b, scratch, counts),
        Rejection::LinearFit(b) => linear_fit(stack, n, b, counts),
        Rejection::Sigma(b) => clip_until_stable(
            stack,
            n,
            b,
            counts,
            |values, scratch| {
                let centre = median_of(values, scratch);
                Some((centre, standard_deviation(values)))
            },
            scratch,
        ),
        Rejection::Mad(b) => clip_until_stable(
            stack,
            n,
            b,
            counts,
            |values, scratch| {
                let centre = median_of(values, scratch);
                Some((centre, mad_about(values, centre, scratch)))
            },
            scratch,
        ),
        Rejection::Winsorized(b) => clip_until_stable(
            stack,
            n,
            b,
            counts,
            |values, scratch| {
                let centre = median_of(values, scratch);
                Some((centre, winsorized_deviation(values, centre, scratch)))
            },
            scratch,
        ),
    }
}

/// Iterated clipping about a centre, with the estimator supplied by the
/// caller. Stops when a pass drops nothing, when the estimate degenerates, or
/// when dropping more would leave fewer than [`MIN_KEPT`] values.
fn clip_until_stable(
    stack: &mut [f32],
    mut n: usize,
    bounds: Bounds,
    counts: &mut Rejected,
    mut estimate: impl FnMut(&[f32], &mut Scratch) -> Option<(f32, f32)>,
    scratch: &mut Scratch,
) -> usize {
    loop {
        let Some((centre, spread)) = estimate(&stack[..n], scratch) else {
            return n;
        };
        // No spread means every value is the centre; there is no outlier to
        // find and the thresholds would reject nothing or everything.
        if spread <= 0.0 || !spread.is_finite() {
            return n;
        }

        let lower = centre - bounds.low * spread;
        let upper = centre + bounds.high * spread;

        // Count first, so a pass that would cut too deep can be abandoned
        // whole rather than leaving the stack half-clipped.
        let survivors = stack[..n]
            .iter()
            .filter(|v| **v >= lower && **v <= upper)
            .count();
        if survivors == n || survivors < MIN_KEPT {
            return n;
        }

        let mut kept = 0;
        for i in 0..n {
            let v = stack[i];
            if v < lower {
                counts.low += 1;
            } else if v > upper {
                counts.high += 1;
            } else {
                stack[kept] = v;
                kept += 1;
            }
        }
        n = kept;
    }
}

/// Rejection at fixed fractions of the median value.
///
/// The bounds are relative to the median *level*, not to a spread — with only
/// a handful of frames there is no spread estimate worth trusting, but the
/// median level is still meaningful. One pass: there is no estimate to refine.
fn percentile(
    stack: &mut [f32],
    n: usize,
    bounds: Bounds,
    scratch: &mut Scratch,
    counts: &mut Rejected,
) -> usize {
    let median = median_of(&stack[..n], scratch);
    // Thresholds are proportional to the level, so a zero level has none.
    if median <= 0.0 {
        return n;
    }

    let lower = median - median * bounds.low;
    let upper = median + median * bounds.high;

    // The MIN_KEPT floor that guards the iterative methods does not apply
    // here. It exists because re-estimating a spread from a dwindling stack
    // measures its own noise — and this method estimates no spread. It is also
    // the method chosen when frames are few, so a floor of four would stop it
    // rejecting anything at all on the stacks it exists for. One survivor is
    // enough; none falls back to the median in the caller.
    let survivors = stack[..n]
        .iter()
        .filter(|v| **v >= lower && **v <= upper)
        .count();
    if survivors == 0 {
        return n;
    }

    let mut kept = 0;
    for i in 0..n {
        let v = stack[i];
        if v < lower {
            counts.low += 1;
        } else if v > upper {
            counts.high += 1;
        } else {
            stack[kept] = v;
            kept += 1;
        }
    }
    kept
}

/// Replaces outliers with the median instead of dropping them, so every frame
/// keeps contributing and the frame count stays constant. Repeats until a pass
/// replaces nothing.
fn sigma_median(
    stack: &mut [f32],
    n: usize,
    bounds: Bounds,
    scratch: &mut Scratch,
    counts: &mut Rejected,
) -> usize {
    loop {
        let centre = median_of(&stack[..n], scratch);
        let spread = standard_deviation(&stack[..n]);
        if spread <= 0.0 || !spread.is_finite() {
            return n;
        }

        let lower = centre - bounds.low * spread;
        let upper = centre + bounds.high * spread;

        let mut replaced = 0;
        for v in stack[..n].iter_mut() {
            if *v < lower {
                counts.low += 1;
                *v = centre;
                replaced += 1;
            } else if *v > upper {
                counts.high += 1;
                *v = centre;
                replaced += 1;
            }
        }
        if replaced == 0 {
            return n;
        }
    }
}

/// Fits a least-squares line through the sorted values and clips on the
/// residual.
///
/// Sorting first is what makes the fit meaningful: against the sorted index, a
/// well-behaved stack is close to a straight line, and an outlier shows as a
/// large residual at one end. Against the original frame order it would be
/// fitting the sequence's own history instead.
fn linear_fit(stack: &mut [f32], mut n: usize, bounds: Bounds, counts: &mut Rejected) -> usize {
    loop {
        stack[..n].sort_by(f32::total_cmp);
        let (intercept, slope) = least_squares_over_index(&stack[..n]);

        // Mean absolute residual rather than an RMS one: the stack still
        // contains the outliers being looked for, and squaring would let them
        // set the threshold that is supposed to exclude them.
        let mut residual = 0.0f64;
        for (i, &v) in stack[..n].iter().enumerate() {
            residual += (v - (slope * i as f32 + intercept)).abs() as f64;
        }
        let spread = (residual / n as f64) as f32;
        if spread <= 0.0 || !spread.is_finite() {
            return n;
        }

        let fitted = |i: usize| slope * i as f32 + intercept;
        let survivors = stack[..n]
            .iter()
            .enumerate()
            .filter(|(i, v)| {
                let f = fitted(*i);
                **v >= f - bounds.low * spread && **v <= f + bounds.high * spread
            })
            .count();
        if survivors == n || survivors < MIN_KEPT {
            return n;
        }

        let mut kept = 0;
        for i in 0..n {
            let v = stack[i];
            let f = fitted(i);
            if v < f - bounds.low * spread {
                counts.low += 1;
            } else if v > f + bounds.high * spread {
                counts.high += 1;
            } else {
                stack[kept] = v;
                kept += 1;
            }
        }
        n = kept;
    }
}

/// Median of `values`, using scratch so the caller's order is untouched.
fn median_of(values: &[f32], scratch: &mut Scratch) -> f32 {
    let n = values.len();
    scratch.work[..n].copy_from_slice(values);
    stats::median_in_place(&mut scratch.work[..n])
}

/// Median absolute deviation about `centre`.
fn mad_about(values: &[f32], centre: f32, scratch: &mut Scratch) -> f32 {
    let n = values.len();
    for (slot, v) in scratch.deviations[..n].iter_mut().zip(values) {
        *slot = (v - centre).abs();
    }
    stats::median_in_place(&mut scratch.deviations[..n])
}

/// Sample standard deviation.
///
/// The sample form, not the population one: at the frame counts this runs at —
/// six to a few hundred — the difference is not negligible, and these values
/// are a sample of what the pixel could have read.
fn standard_deviation(values: &[f32]) -> f32 {
    let n = values.len();
    if n < 2 {
        return 0.0;
    }
    let mut sum = 0.0f64;
    for &v in values {
        sum += v as f64;
    }
    let mean = sum / n as f64;

    let mut acc = 0.0f64;
    for &v in values {
        let d = v as f64 - mean;
        acc += d * d;
    }
    (acc / (n - 1) as f64).sqrt() as f32
}

/// Standard deviation of the values after their tails are repeatedly pulled in
/// to `centre ± 1.5σ`, iterated to convergence and rescaled for consistency.
///
/// The point is that an ordinary standard deviation is inflated by exactly the
/// outliers it is being used to detect. Clamping the tails to a fixed multiple
/// of the current estimate and re-measuring converges on a spread that
/// describes the bulk of the distribution; the 1.134 factor puts that back on
/// the same scale as an ordinary sigma for normal data.
fn winsorized_deviation(values: &[f32], centre: f32, scratch: &mut Scratch) -> f32 {
    let n = values.len();
    let mut sigma = standard_deviation(values);
    if sigma <= 0.0 || !sigma.is_finite() {
        return 0.0;
    }

    // Bounded: the iteration converges quickly in practice, and a stack that
    // somehow oscillates must not spin here once per pixel.
    for _ in 0..32 {
        let lower = centre - WINSOR_LIMIT * sigma;
        let upper = centre + WINSOR_LIMIT * sigma;
        for (slot, v) in scratch.work[..n].iter_mut().zip(values) {
            *slot = v.clamp(lower, upper);
        }

        let next = WINSOR_CONSISTENCY * standard_deviation(&scratch.work[..n]);
        if next <= 0.0 || !next.is_finite() {
            return 0.0;
        }
        let converged = (next - sigma).abs() <= sigma * WINSOR_TOLERANCE;
        sigma = next;
        if converged {
            break;
        }
    }
    sigma
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
