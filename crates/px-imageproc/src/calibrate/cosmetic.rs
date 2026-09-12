//! Hot and cold pixel correction (ADR 007, P4-T4).
//!
//! Sensors have pixels that read high or low regardless of what fell on them.
//! A master dark shows them plainly — it is an image of the sensor's own
//! behaviour — so they are found there once and then patched in every light.
//!
//! Hot pixels are replaced by the mean of their neighbours and cold ones by
//! the median. That asymmetry is deliberate: a hot pixel is usually isolated,
//! so its neighbours are clean and a mean is the better estimate, whereas cold
//! pixels cluster into columns and blocks where a mean would drag the
//! replacement toward the defect it is meant to hide.
//!
//! On a colour sensor every step is taken in strides of two so the neighbours
//! sampled carry the same filter. Averaging a red pixel with its green
//! neighbours would produce a colour artifact exactly where a defect was.

use rayon::prelude::*;

use crate::frame::Frame;
use crate::stats;

/// Sigma thresholds for deviant-pixel detection, relative to the master
/// dark's own median and standard deviation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    /// Pixels below `median - cold * sigma`. `None` disables cold detection.
    pub cold: Option<f32>,
    /// Pixels above `median + hot * sigma`. `None` disables hot detection.
    pub hot: Option<f32>,
}

impl Thresholds {
    /// Siril's defaults for automatic detection from a master dark.
    pub fn default_from_dark() -> Self {
        Self {
            cold: Some(3.0),
            hot: Some(3.0),
        }
    }
}

/// Positions of the defective pixels, as linear indices into one channel.
#[derive(Debug, Clone, Default)]
pub struct DeviantPixels {
    pub cold: Vec<u32>,
    pub hot: Vec<u32>,
}

impl DeviantPixels {
    pub fn total(&self) -> usize {
        self.cold.len() + self.hot.len()
    }

    pub fn is_empty(&self) -> bool {
        self.total() == 0
    }
}

/// Finds deviant pixels in a master dark, from its first channel.
pub fn find_deviant_pixels(dark: &Frame, thresholds: Thresholds) -> DeviantPixels {
    let data = dark.channel(0);
    let (_, _, sigma) = stats::mean_sigma(data);
    let median = stats::median(data);

    // A pixel cannot be darker than nothing, so the cold threshold is floored
    // at zero; the hot one is capped at saturation for the same reason.
    let cold_threshold = thresholds
        .cold
        .map(|k| (median - k * sigma as f32).max(0.0));
    let hot_threshold = thresholds.hot.map(|k| (median + k * sigma as f32).min(1.0));

    let mut result = DeviantPixels::default();
    for (i, &v) in data.iter().enumerate() {
        // Hot is tested first: with thresholds this tight on a degenerate
        // frame the two windows can overlap, and a pixel must land in exactly
        // one list or it would be corrected twice.
        if hot_threshold.is_some_and(|t| v >= t) {
            result.hot.push(i as u32);
        } else if cold_threshold.is_some_and(|t| v <= t) {
            result.cold.push(i as u32);
        }
    }
    result
}

/// Patches the listed pixels in every channel of `frame`.
///
/// `is_cfa` makes the neighbourhood sampling stride by two so replacements
/// come from same-colour pixels.
pub fn correct(frame: &mut Frame, deviants: &DeviantPixels, is_cfa: bool) {
    if deviants.is_empty() {
        return;
    }
    let (width, height) = (frame.width(), frame.height());
    let channels = frame.channels();

    for channel in 0..channels {
        let data = frame.channel_mut(channel);

        // Replacements are computed from the original values and applied
        // afterwards, so two adjacent defects cannot feed each other.
        let hot: Vec<(u32, f32)> = deviants
            .hot
            .par_iter()
            .map(|&i| {
                let (x, y) = ((i as usize % width), (i as usize / width));
                (i, neighbour_mean(data, x, y, width, height, is_cfa))
            })
            .collect();
        let cold: Vec<(u32, f32)> = deviants
            .cold
            .par_iter()
            .map(|&i| {
                let (x, y) = ((i as usize % width), (i as usize / width));
                (i, neighbour_median(data, x, y, width, height, is_cfa))
            })
            .collect();

        for (i, v) in hot.into_iter().chain(cold) {
            data[i as usize] = v;
        }
    }
}

/// Mean of the eight neighbours around `(x, y)`, excluding the centre.
fn neighbour_mean(
    data: &[f32],
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    is_cfa: bool,
) -> f32 {
    let step = if is_cfa { 2i64 } else { 1 };
    let radius = step;

    let mut sum = 0.0f32;
    let mut n = 0usize;
    let mut dy = -radius;
    while dy <= radius {
        let ny = y as i64 + dy;
        if ny >= 0 && ny < height as i64 {
            let mut dx = -radius;
            while dx <= radius {
                let nx = x as i64 + dx;
                if nx >= 0 && nx < width as i64 && !(dx == 0 && dy == 0) {
                    sum += data[ny as usize * width + nx as usize];
                    n += 1;
                }
                dx += step;
            }
        }
        dy += step;
    }

    if n == 0 {
        data[y * width + x]
    } else {
        sum / n as f32
    }
}

/// Median of the neighbourhood around `(x, y)`, excluding the centre.
///
/// The window reaches two samples out in each direction — five across — so
/// that a cold *column*, which is the common case, still has clean pixels to
/// draw from.
fn neighbour_median(
    data: &[f32],
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    is_cfa: bool,
) -> f32 {
    let step = if is_cfa { 2i64 } else { 1 };
    let radius = 2 * step;

    let mut values: Vec<f32> = Vec::with_capacity(25);
    let mut dy = -radius;
    while dy <= radius {
        let ny = y as i64 + dy;
        if ny >= 0 && ny < height as i64 {
            let mut dx = -radius;
            while dx <= radius {
                let nx = x as i64 + dx;
                if nx >= 0 && nx < width as i64 && !(dx == 0 && dy == 0) {
                    values.push(data[ny as usize * width + nx as usize]);
                }
                dx += step;
            }
        }
        dy += step;
    }

    if values.is_empty() {
        data[y * width + x]
    } else {
        stats::median_in_place(&mut values)
    }
}
