//! Choosing how much of the master dark to subtract (ADR 007, P4-T3).
//!
//! A master dark taken at a different exposure or temperature than the lights
//! does not match them pixel for pixel, but it is usually still the right
//! *shape* — thermal signal scales roughly linearly with both. Scaling it by
//! one factor before subtracting recovers most of the benefit.
//!
//! Two ways to pick that factor. If the exposures are trustworthy, the ratio
//! of exposure times is the physical answer and costs nothing. Otherwise it is
//! found by search: the right factor is the one that leaves the calibrated
//! frame quietest, since under-subtracting leaves thermal structure behind and
//! over-subtracting carves it out inverted, both of which add variance.

use crate::calibrate::ops;
use crate::frame::Frame;
use crate::stats;

/// How the dark's scaling factor is chosen.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DarkScaling {
    /// Subtract the dark as-is.
    None,
    /// Ratio of the light's exposure to the dark's, from their headers.
    Exposure,
    /// Search for the factor that minimizes noise in the result.
    MinimizeNoise,
}

/// Side of the square sampled when evaluating noise.
///
/// The search evaluates noise many times, and doing that over 61 million
/// pixels would cost more than the calibration it is tuning. A square from the
/// centre is representative of the thermal signal being cancelled and small
/// enough to measure in microseconds.
const SAMPLE_SIZE: usize = 512;

/// `(sqrt(5) - 1) / 2`. Each step of a golden-section search discards the same
/// fraction of the interval, so one of the two probes from the previous step
/// can be reused — halving the evaluations a ternary search would need.
const GOLDEN_RATIO: f32 = 0.618_034;

/// Bracket searched for the factor. Zero is "do not subtract"; beyond twice
/// the dark, the frame is being carved into rather than calibrated.
const SEARCH_LOW: f32 = 0.0;
const SEARCH_HIGH: f32 = 2.0;

/// Stops when the bracket is this narrow. Finer than the factor is
/// meaningful — the dark is itself an estimate.
const TOLERANCE: f32 = 0.001;

/// Ratio of the light's exposure to the dark's, if both headers carry one.
pub fn exposure_ratio(light: &Frame, dark: &Frame) -> Option<f32> {
    let exposure = |f: &Frame| {
        f.header()
            .get_f64("EXPTIME")
            .or_else(|| f.header().get_f64("EXPOSURE"))
            .filter(|v| *v > 0.0)
    };
    Some((exposure(light)? / exposure(dark)?) as f32)
}

/// Finds the dark scaling factor that leaves the calibrated frame quietest.
///
/// Returns a factor in `[0, 2]`. A completely flat sample — a synthetic frame,
/// or a region with no thermal signal at all — has no minimum to find, and
/// yields `1.0`.
pub fn search_optimal_factor(light: &Frame, dark: &Frame) -> f32 {
    let sample = Sample::centred(light, dark);

    let (mut a, mut b) = (SEARCH_LOW, SEARCH_HIGH);
    let mut c = b - GOLDEN_RATIO * (b - a);
    let mut d = a + GOLDEN_RATIO * (b - a);
    let mut fc = sample.noise_at(c);
    let mut fd = sample.noise_at(d);

    // Equal at both probes means the noise does not respond to the factor at
    // all, so there is nothing to optimize.
    if fc == fd {
        return 1.0;
    }

    while (c - d).abs() > TOLERANCE {
        if fc < fd {
            b = d;
            d = c;
            fd = fc;
            c = b - GOLDEN_RATIO * (b - a);
            fc = sample.noise_at(c);
        } else {
            a = c;
            c = d;
            fc = fd;
            d = a + GOLDEN_RATIO * (b - a);
            fd = sample.noise_at(d);
        }
    }

    (a + b) * 0.5
}

/// A matching square cut from the light and the dark, so the search can
/// evaluate a candidate factor without touching either full frame.
struct Sample {
    light: Vec<f32>,
    dark: Vec<f32>,
    width: usize,
    height: usize,
    channels: usize,
}

impl Sample {
    fn centred(light: &Frame, dark: &Frame) -> Self {
        let width = SAMPLE_SIZE.min(light.width());
        let height = SAMPLE_SIZE.min(light.height());
        let x = (light.width() - width) / 2;
        let y = (light.height() - height) / 2;
        let channels = light.channels();

        let cut = |frame: &Frame| {
            let mut out = Vec::with_capacity(width * height * channels);
            for c in 0..channels {
                let plane = frame.channel(c);
                for row in y..y + height {
                    let start = row * frame.width() + x;
                    out.extend_from_slice(&plane[start..start + width]);
                }
            }
            out
        };

        Self {
            light: cut(light),
            dark: cut(dark),
            width,
            height,
            channels,
        }
    }

    /// Noise left in the sample after subtracting `factor` times the dark,
    /// summed over channels.
    fn noise_at(&self, factor: f32) -> f32 {
        let mut scaled = self.dark.clone();
        ops::scale(&mut scaled, factor);

        let mut residual = self.light.clone();
        ops::subtract_frame(&mut residual, &scaled);

        let plane = self.width * self.height;
        (0..self.channels)
            .map(|c| {
                let (_, _, sigma) = stats::mean_sigma(&residual[c * plane..(c + 1) * plane]);
                sigma as f32
            })
            .sum()
    }
}
