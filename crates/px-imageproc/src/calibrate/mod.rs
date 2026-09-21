//! Turning a raw frame into a calibrated one (ADR 007, Phase 4).
//!
//! Three corrections, always in this order, because each one assumes the
//! previous has already happened:
//!
//! 1. **Bias** — the sensor's read-out pedestal, present in every exposure
//!    regardless of length. Subtracted first so nothing downstream carries it.
//! 2. **Dark** — thermal signal accumulated over the exposure. A master dark
//!    already contains the bias, so when both are used the dark subtraction
//!    removes it a second time; that is why calibrating with a bias *and* an
//!    un-debiased dark over-subtracts, and why masters are built to match.
//! 3. **Flat** — the optical system's per-pixel response: vignetting, dust,
//!    filter transmission. A division, not a subtraction, and it must come
//!    last because it is multiplicative on *signal*, not on the additive
//!    pedestals above.

pub mod cosmetic;
pub mod dark_optimize;
pub mod ops;

use crate::bayer::BayerPattern;
use crate::error::ImageProcError;
use crate::frame::Frame;

pub use cosmetic::{DeviantPixels, Thresholds};
pub use dark_optimize::DarkScaling;

/// The master frames to calibrate against. All optional — a session with no
/// flats still benefits from bias and dark removal.
#[derive(Default)]
pub struct Masters<'a> {
    pub bias: Option<&'a Frame>,
    /// A constant pedestal to subtract instead of a master bias, in the unit
    /// range. Takes precedence over `bias` when both are set.
    ///
    /// Useful when no bias frames exist but the camera's offset is known: it
    /// removes the pedestal without adding the master's own read noise, which
    /// a synthetic level cannot have.
    pub bias_level: Option<f32>,
    pub dark: Option<&'a Frame>,
    pub flat: Option<&'a Frame>,
}

impl Masters<'_> {
    fn is_empty(&self) -> bool {
        self.bias.is_none()
            && self.bias_level.is_none()
            && self.dark.is_none()
            && self.flat.is_none()
    }
}

/// How to calibrate.
#[derive(Debug, Clone, Copy)]
pub struct CalibrationOptions {
    pub dark_scaling: DarkScaling,
    /// Detect and patch deviant pixels using the master dark. Requires a dark.
    pub cosmetic: Option<Thresholds>,
    /// Equalize the master flat's colour channels before dividing by it.
    /// Only meaningful for a CFA sensor.
    pub equalize_cfa: bool,
    /// The sensor's CFA layout, which makes cosmetic correction and CFA
    /// equalization sample same-coloured neighbours.
    pub pattern: BayerPattern,
}

impl Default for CalibrationOptions {
    fn default() -> Self {
        Self {
            dark_scaling: DarkScaling::None,
            cosmetic: None,
            equalize_cfa: false,
            pattern: BayerPattern::None,
        }
    }
}

/// What calibrating a frame did.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CalibrationReport {
    /// Factor the dark was scaled by before subtraction.
    pub dark_factor: Option<f32>,
    /// Fraction of pixels left negative after calibration. A fraction of a
    /// percent is the low tail of sky noise; a large fraction means the
    /// masters do not match the lights.
    pub negative_ratio: f32,
    /// Pixels destroyed by the clamps: driven past `-1.0` and zeroed, or past
    /// `1.0` and saturated.
    ///
    /// Both are irreversible, so they are counted rather than left implicit.
    /// On a healthy frame these are a handful of sensor defects and bright
    /// stars; a large count means something is wrong with the masters.
    pub floored_pixels: usize,
    pub saturated_pixels: usize,
    pub hot_pixels_corrected: usize,
    pub cold_pixels_corrected: usize,
    /// Level the flat was normalized to before dividing.
    pub flat_level: Option<f32>,
}

/// Level a master flat is normalized to before division.
///
/// Measured over the central third of the frame rather than the whole thing,
/// because vignetting makes the corners darker than the optics alone would
/// explain and would drag the level down.
pub fn flat_normalization_level(flat: &Frame) -> f32 {
    let (w, h) = (flat.width(), flat.height());
    let (x, y) = (w / 3, h / 3);
    ops::region_mean(flat.channel(0), w, x, y, w / 3, h / 3) as f32
}

/// Rescales a CFA flat's red and blue pixels so all three colours share the
/// green channel's mean level.
///
/// Without this the flat carries the sensor's colour response as well as the
/// optics', and dividing by it would neutralize the colour balance of every
/// light — leaving a technically flat but colourless result.
pub fn equalize_cfa(flat: &mut Frame, pattern: BayerPattern) -> Result<(), ImageProcError> {
    if pattern == BayerPattern::None {
        return Err(ImageProcError::Processing(
            "CFA equalization needs a Bayer pattern".to_string(),
        ));
    }
    let (width, height) = (flat.width(), flat.height());
    let data = flat.channel_mut(0);

    let mut sums = [0.0f64; 3];
    let mut counts = [0usize; 3];
    for y in 0..height {
        for x in 0..width {
            let c = cfa_color(pattern, x, y);
            sums[c] += data[y * width + x] as f64;
            counts[c] += 1;
        }
    }

    let mean = |c: usize| {
        if counts[c] == 0 {
            0.0
        } else {
            sums[c] / counts[c] as f64
        }
    };
    let (red, green, blue) = (mean(0), mean(1), mean(2));
    if green <= 0.0 {
        return Err(ImageProcError::Processing(
            "CFA equalization: the flat's green channel is empty".to_string(),
        ));
    }

    // Green is the reference because it has twice the sample count of either
    // other colour, so its mean is the most stable of the three.
    let red_coeff = (red / green) as f32;
    let blue_coeff = (blue / green) as f32;

    for y in 0..height {
        for x in 0..width {
            let i = y * width + x;
            match cfa_color(pattern, x, y) {
                0 if red_coeff > 0.0 => data[i] /= red_coeff,
                2 if blue_coeff > 0.0 => data[i] /= blue_coeff,
                _ => {}
            }
        }
    }
    Ok(())
}

/// Colour index at `(x, y)`: 0 red, 1 green, 2 blue.
fn cfa_color(pattern: BayerPattern, x: usize, y: usize) -> usize {
    let tile = match pattern {
        BayerPattern::Rggb => [[0, 1], [1, 2]],
        BayerPattern::Bggr => [[2, 1], [1, 0]],
        BayerPattern::Gbrg => [[1, 2], [0, 1]],
        BayerPattern::Grbg => [[1, 0], [2, 1]],
        BayerPattern::None => [[1, 1], [1, 1]],
    };
    tile[y % 2][x % 2]
}

/// Calibrates `frame` in place.
///
/// The frame keeps its own header, so the caller can stamp provenance before
/// writing.
pub fn calibrate(
    frame: &mut Frame,
    masters: &Masters<'_>,
    options: &CalibrationOptions,
) -> Result<CalibrationReport, ImageProcError> {
    if masters.is_empty() {
        return Ok(CalibrationReport::default());
    }
    let dims = frame.dims();
    let mut report = CalibrationReport::default();

    // Deviant pixels are located in the master dark before anything is
    // subtracted from it, and patched in the light at the end.
    let deviants = match (options.cosmetic, masters.dark) {
        (Some(thresholds), Some(dark)) => {
            check_dimensions(frame, dark, "dark")?;
            Some(cosmetic::find_deviant_pixels(dark, thresholds))
        }
        (Some(_), None) => {
            return Err(ImageProcError::Processing(
                "cosmetic correction needs a master dark to find defects in".to_string(),
            ));
        }
        _ => None,
    };

    if let Some(level) = masters.bias_level {
        ops::subtract_scalar(frame.data_mut(), level);
    } else if let Some(bias) = masters.bias {
        check_dimensions(frame, bias, "bias")?;
        let subtraction = ops::subtract_frame(frame.data_mut(), bias.data());
        report.negative_ratio = subtraction.negative_ratio();
        report.floored_pixels += subtraction.floored;
        report.saturated_pixels += subtraction.saturated;
    }

    if let Some(dark) = masters.dark {
        check_dimensions(frame, dark, "dark")?;

        let factor = match options.dark_scaling {
            DarkScaling::None => None,
            DarkScaling::Exposure => dark_optimize::exposure_ratio(frame, dark),
            DarkScaling::MinimizeNoise => Some(dark_optimize::search_optimal_factor(frame, dark)),
        };

        let subtraction = match factor {
            Some(k) => {
                report.dark_factor = Some(k);
                let mut scaled = dark.data().to_vec();
                ops::scale(&mut scaled, k);
                ops::subtract_frame(frame.data_mut(), &scaled)
            }
            None => ops::subtract_frame(frame.data_mut(), dark.data()),
        };
        report.negative_ratio = subtraction.negative_ratio();
        report.floored_pixels += subtraction.floored;
        report.saturated_pixels += subtraction.saturated;
    }

    if let Some(flat) = masters.flat {
        check_dimensions(frame, flat, "flat")?;

        // Equalization changes the flat, so work on a copy rather than
        // mutating a master the caller may reuse for the next frame.
        let equalized;
        let flat = if options.equalize_cfa {
            let mut copy = flat.clone();
            equalize_cfa(&mut copy, options.pattern)?;
            equalized = copy;
            &equalized
        } else {
            flat
        };

        let level = flat_normalization_level(flat);
        if level <= 0.0 {
            return Err(ImageProcError::Processing(
                "master flat has no signal in its central region".to_string(),
            ));
        }
        report.flat_level = Some(level);
        let division = ops::divide_frame(frame.data_mut(), flat.data(), level);
        // The flat runs last, so its view of the frame is the final one.
        report.negative_ratio = division.negative_ratio();
        report.floored_pixels += division.floored;
        report.saturated_pixels += division.saturated;
    }

    if let Some(deviants) = deviants {
        let is_cfa = options.pattern != BayerPattern::None;
        report.hot_pixels_corrected = deviants.hot.len();
        report.cold_pixels_corrected = deviants.cold.len();
        cosmetic::correct(frame, &deviants, is_cfa);
    }

    debug_assert_eq!(frame.dims(), dims);
    Ok(report)
}

fn check_dimensions(frame: &Frame, master: &Frame, what: &str) -> Result<(), ImageProcError> {
    if frame.dims() != master.dims() {
        return Err(ImageProcError::Processing(format!(
            "master {what} is {}x{}x{} but the frame is {}x{}x{}",
            master.width(),
            master.height(),
            master.channels(),
            frame.width(),
            frame.height(),
            frame.channels()
        )));
    }
    Ok(())
}
