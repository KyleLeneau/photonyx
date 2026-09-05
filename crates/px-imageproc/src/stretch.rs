//! STF (screen transfer function) autostretch (ADR 006, Phase 9, P9-T5).
//!
//! This is PixInsight's midtone transfer function autostretch, the same
//! algorithm ASIAIR/Siril/PixInsight previews use: shadows/highlights
//! clipping points from the median and MAD (median absolute deviation,
//! scaled by the usual 1.4826 normal-consistency constant), and a midtones
//! balance chosen so the median maps to a fixed target value. Matches
//! `astroimage`'s `compute_stretch_params`/`apply_stretch` bit-for-bit
//! (P9-T6 parity target).

use rayon::prelude::*;

/// Quickselect: finds the k-th smallest element in-place (partial sort).
/// Median-of-three pivot, insertion sort for small partitions.
fn quickselect(arr: &mut [f32], k: usize) -> f32 {
    if arr.is_empty() {
        return 0.0;
    }
    if arr.len() == 1 {
        return arr[0];
    }
    let mut left = 0usize;
    let mut right = arr.len() - 1;

    while left < right {
        if right - left < 3 {
            for i in (left + 1)..=right {
                let mut j = i;
                while j > left && arr[j - 1] > arr[j] {
                    arr.swap(j - 1, j);
                    j -= 1;
                }
            }
            return arr[k];
        }

        let mid = left + (right - left) / 2;
        if arr[mid] < arr[left] {
            arr.swap(left, mid);
        }
        if arr[right] < arr[left] {
            arr.swap(left, right);
        }
        if arr[right] < arr[mid] {
            arr.swap(mid, right);
        }
        let pivot = arr[mid];
        arr.swap(mid, right - 1);

        let mut i = left;
        let mut j = right - 1;
        loop {
            i += 1;
            while arr[i] < pivot {
                i += 1;
            }
            j -= 1;
            while arr[j] > pivot {
                j -= 1;
            }
            if i >= j {
                break;
            }
            arr.swap(i, j);
        }
        arr.swap(i, right - 1);

        if i == k {
            return arr[k];
        } else if i > k {
            right = i - 1;
        } else {
            left = i + 1;
        }
    }

    arr[k]
}

fn find_median(data: &mut [f32]) -> f32 {
    let k = data.len() / 2;
    quickselect(data, k)
}

/// Shadows/highlights clip points and midtones balance, normalized to
/// `[0, 1]` against `max_input`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StretchParams {
    pub shadows: f32,
    pub highlights: f32,
    pub midtones: f32,
}

/// A capped, strided sample keeps the median/MAD computation's cost
/// independent of image size for the large frames this stretch runs on.
const MAX_SAMPLES: usize = 500_000;

fn sample(data: &[f32]) -> Vec<f32> {
    if data.len() <= MAX_SAMPLES {
        data.to_vec()
    } else {
        let step = data.len() / MAX_SAMPLES;
        (0..MAX_SAMPLES).map(|i| data[i * step]).collect()
    }
}

/// Computes STF autostretch parameters from `data`, normalized against
/// `max_input` (the assumed native full-scale value — `65536.0` for 16-bit
/// integer data, matching `astroimage`'s convention regardless of the
/// pixel's actual storage type).
pub fn compute_stretch_params(data: &[f32], max_input: f32) -> StretchParams {
    let mut samples = sample(data);
    let median = find_median(&mut samples);

    let mut deviations: Vec<f32> = samples.iter().map(|&v| (v - median).abs()).collect();
    let madn = 1.4826 * find_median(&mut deviations);

    let norm_median = median / max_input;
    let norm_madn = madn / max_input;
    let upper_half = norm_median > 0.5;

    let shadows = if upper_half || norm_madn == 0.0 {
        0.0
    } else {
        (norm_median + (-2.8 * norm_madn)).clamp(0.0, 1.0)
    };

    let highlights = if !upper_half || norm_madn == 0.0 {
        1.0
    } else {
        (norm_median - (-2.8 * norm_madn)).clamp(0.0, 1.0)
    };

    // Midtones Transfer Function (PixInsight STF): pick `m` so that the
    // median maps to a fixed target `b` (here 0.25) on the appropriate side
    // of the [shadows, highlights] range.
    let b = 0.25f32;
    let (x, m) = if !upper_half {
        (norm_median - shadows, b)
    } else {
        (b, highlights - norm_median)
    };

    let midtones = if x == 0.0 {
        0.0
    } else if x == m {
        0.5
    } else if x == 1.0 {
        1.0
    } else {
        ((m - 1.0) * x) / ((2.0 * m - 1.0) * x - m)
    };

    StretchParams {
        shadows,
        highlights,
        midtones,
    }
}

/// Precomputed coefficients for [`stretch_pixel`]: folds the midtones
/// transfer function and the `[0, max_input] -> [0, 255]` scaling into a
/// single division per pixel.
#[derive(Debug, Clone, Copy)]
pub struct StretchCoeffs {
    pub native_shadows: f32,
    pub native_highlights: f32,
    pub k1: f32,
    pub k2: f32,
    pub midtones: f32,
}

impl StretchCoeffs {
    pub fn from_params(params: StretchParams, max_input: f32) -> Self {
        let hs_range_factor = if params.highlights == params.shadows {
            1.0
        } else {
            1.0 / (params.highlights - params.shadows)
        };
        StretchCoeffs {
            native_shadows: params.shadows * max_input,
            native_highlights: params.highlights * max_input,
            k1: (params.midtones - 1.0) * hs_range_factor * 255.0 / max_input,
            k2: (2.0 * params.midtones - 1.0) * hs_range_factor / max_input,
            midtones: params.midtones,
        }
    }

    /// Computes stretch parameters from `channel_data` and folds them into
    /// coefficients in one call.
    pub fn compute(channel_data: &[f32], max_input: f32) -> Self {
        Self::from_params(compute_stretch_params(channel_data, max_input), max_input)
    }
}

#[inline]
fn stretch_pixel(input: f32, c: &StretchCoeffs) -> u8 {
    let out = if input < c.native_shadows {
        0.0f32
    } else if input >= c.native_highlights {
        255.0f32
    } else {
        let input_floored = input - c.native_shadows;
        (input_floored * c.k1) / (input_floored * c.k2 - c.midtones)
    };
    out.clamp(0.0, 255.0) as u8
}

/// Applies the stretch to `channel_data`, writing into `output` at positions
/// `output_offset + i * stride` (so a single interleaved RGB buffer can be
/// filled one channel at a time with `stride = 3`).
pub fn apply_stretch(
    channel_data: &[f32],
    output: &mut [u8],
    output_offset: usize,
    stride: usize,
    coeffs: &StretchCoeffs,
) {
    for (i, &input) in channel_data.iter().enumerate() {
        output[output_offset + i * stride] = stretch_pixel(input, coeffs);
    }
}

/// Computes per-channel stretch coefficients in parallel — one channel's
/// median/MAD computation (including quickselect) is independent of the
/// others.
pub fn compute_stretch_coeffs_per_channel(
    float_data: &[f32],
    num_channels: usize,
    channel_size: usize,
    max_input: f32,
) -> Vec<StretchCoeffs> {
    (0..num_channels)
        .into_par_iter()
        .map(|c| {
            let ch = &float_data[c * channel_size..(c + 1) * channel_size];
            StretchCoeffs::compute(ch, max_input)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_of_odd_length() {
        let mut v = vec![5.0, 1.0, 3.0];
        assert_eq!(find_median(&mut v), 3.0);
    }

    #[test]
    fn constant_data_clips_everything_to_extremes() {
        // madn == 0 for constant data: shadows -> 0, highlights -> 1.
        let data = vec![100.0f32; 64];
        let params = compute_stretch_params(&data, 65536.0);
        assert_eq!(params.shadows, 0.0);
        assert_eq!(params.highlights, 1.0);
    }

    #[test]
    fn dim_background_stretches_up_to_midgray() {
        // A dark-sky-like histogram: mostly low background with a bright tail.
        let mut data = vec![500.0f32; 900];
        data.extend(std::iter::repeat_n(40000.0f32, 100));
        let coeffs = StretchCoeffs::compute(&data, 65536.0);
        // Background pixels should land near black, bright tail near white.
        let bg = stretch_pixel(500.0, &coeffs);
        let bright = stretch_pixel(40000.0, &coeffs);
        assert!(bg < bright);
        assert!(bright > 200);
    }

    #[test]
    fn stride_writes_into_interleaved_buffer() {
        // A clear background/highlight split so shadows/highlights clip
        // apart, rather than a degenerate 2-sample median.
        let mut data = vec![500.0f32; 900];
        data.extend(std::iter::repeat_n(40000.0f32, 100));
        let coeffs = StretchCoeffs::compute(&data, 65536.0);

        let channel_data = [500.0f32, 40000.0f32];
        let mut out = vec![0u8; 6]; // 2 pixels, RGB
        apply_stretch(&channel_data, &mut out, 1, 3, &coeffs); // write into G channel
        assert_eq!(out[0], 0);
        assert_eq!(out[3], 0);
        assert_eq!(out[2], 0);
        assert_eq!(out[5], 0);
        assert!(
            out[1] < out[4],
            "background should stretch darker than highlight"
        );
    }
}
