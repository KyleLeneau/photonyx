//! Pixel arithmetic for calibration (ADR 007, P4-T1).
//!
//! Everything works in the unit range, and the clamping rule is shared by
//! every frame-to-frame operation:
//!
//! - above `1.0` clamps to `1.0` — a saturated pixel stays saturated
//! - below `-1.0` becomes `0.0` — that far negative is not signal, it is a
//!   mismatched calibration frame, and zero marks it as absent
//! - between `-1.0` and `0.0` is **left negative**
//!
//! That last case looks like a bug and is not. Subtracting a master dark from
//! a light leaves the sky background scattered around zero; clipping the
//! negative half would bias every subsequent statistic upward and
//! systematically brighten the stack. The values stay signed until the stack
//! is written.
//!
//! Scalar operations do not clamp at all, because they are used for interior
//! steps — scaling a dark by an optimization factor — where an intermediate
//! outside the range is meaningful.

use rayon::prelude::*;

/// Samples per parallel chunk. Large enough that the coordination cost is
/// irrelevant, small enough to keep every core fed on a 61 MP frame.
const CHUNK: usize = 65_536;

/// What an operation did to the pixels it touched.
///
/// The two clamp counts matter because both are *destructive*: a saturated
/// pixel and a floored one have lost their original value. On a healthy frame
/// they are a rounding error — a handful of stars and a handful of sensor
/// defects. If either grows, the masters do not match the lights, and the
/// counts are how a caller finds that out.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct OpReport {
    /// Pixels left below zero. Not destructive — see the module docs.
    pub negative: usize,
    /// Pixels that fell below `-1.0` and were therefore zeroed.
    pub floored: usize,
    /// Pixels that rose above `1.0` and were therefore clamped.
    pub saturated: usize,
    pub total: usize,
}

impl OpReport {
    pub fn negative_ratio(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.negative as f32 / self.total as f32
        }
    }

    fn merge(&mut self, other: OpReport) {
        self.negative += other.negative;
        self.floored += other.floored;
        self.saturated += other.saturated;
        self.total += other.total;
    }
}

/// Clamp shared by the frame-to-frame operations, counting what it destroys.
/// See the module docs for why small negatives survive.
#[inline]
fn clamp(v: f32, report: &mut OpReport) -> f32 {
    report.total += 1;
    if v > 1.0 {
        report.saturated += 1;
        1.0
    } else if v < -1.0 {
        report.floored += 1;
        0.0
    } else {
        if v < 0.0 {
            report.negative += 1;
        }
        v
    }
}

/// `target -= other`, elementwise, clamped.
pub fn subtract_frame(target: &mut [f32], other: &[f32]) -> OpReport {
    assert_eq!(target.len(), other.len(), "frames differ in size");

    target
        .par_chunks_mut(CHUNK)
        .zip(other.par_chunks(CHUNK))
        .map(|(t, o)| {
            let mut report = OpReport::default();
            for (a, b) in t.iter_mut().zip(o) {
                *a = clamp(*a - *b, &mut report);
            }
            report
        })
        .reduce(OpReport::default, |mut a, b| {
            a.merge(b);
            a
        })
}

/// `target -= value`, elementwise. Not clamped.
pub fn subtract_scalar(target: &mut [f32], value: f32) {
    target.par_chunks_mut(CHUNK).for_each(|chunk| {
        for v in chunk {
            *v -= value;
        }
    });
}

/// `target *= factor`, elementwise. Not clamped.
pub fn scale(target: &mut [f32], factor: f32) {
    target.par_chunks_mut(CHUNK).for_each(|chunk| {
        for v in chunk {
            *v *= factor;
        }
    });
}

/// `target = (target / divisor) * factor`, elementwise, clamped.
///
/// A zero divisor yields zero rather than an infinity: a flat that reads zero
/// somewhere has no information about that pixel's response, so the result is
/// marked absent instead of exploding.
pub fn divide_frame(target: &mut [f32], divisor: &[f32], factor: f32) -> OpReport {
    assert_eq!(target.len(), divisor.len(), "frames differ in size");

    target
        .par_chunks_mut(CHUNK)
        .zip(divisor.par_chunks(CHUNK))
        .map(|(t, d)| {
            let mut report = OpReport::default();
            for (a, b) in t.iter_mut().zip(d) {
                let divided = if *b == 0.0 { 0.0 } else { *a / *b };
                *a = clamp(divided * factor, &mut report);
            }
            report
        })
        .reduce(OpReport::default, |mut a, b| {
            a.merge(b);
            a
        })
}

/// Mean of a rectangular region of one channel, ignoring nulls.
pub fn region_mean(data: &[f32], width: usize, x: usize, y: usize, w: usize, h: usize) -> f64 {
    let mut sum = 0.0f64;
    let mut count = 0usize;
    for row in y..y + h {
        let start = row * width + x;
        for &v in &data[start..start + w] {
            if v != 0.0 && !v.is_nan() {
                sum += v as f64;
                count += 1;
            }
        }
    }
    if count == 0 { 0.0 } else { sum / count as f64 }
}
