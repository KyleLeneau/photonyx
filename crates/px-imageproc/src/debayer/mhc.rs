//! Gradient-corrected linear demosaicing (ADR 007, Phase 6).
//!
//! Implements the method of Malvar, He and Cutler, *High-Quality Linear
//! Interpolation for Demosaicing of Bayer-Patterned Color Images*
//! (ICASSP 2004; Microsoft Research MSR-TR-2004-92). The paper states the
//! filters outright, which is what makes it implementable from its published
//! description.
//!
//! The idea is short. Bilinear interpolation of a missing colour is fine on
//! flat areas and smears across edges, because it ignores what the *measured*
//! channel at that pixel is doing. At a red pixel the red value is known
//! exactly, and red correlates strongly with green across an edge — so the
//! local second derivative of red estimates the green one. Adding exactly that
//! correction, weighted by a gain fitted on a reference image set, gives four
//! fixed 5×5 FIR filters: cheap, and a large quality gain over bilinear.
//!
//! The filters are transcribed below as tap lists rather than folded into
//! algebra, so each can be read off against the paper's figure.
//!
//! **This is not RCD**, the method Siril uses by default. RCD has no published
//! specification independent of its implementation, so it cannot be written
//! clean-room (see the ADR's licensing section). The gap between the two is
//! measured in `tests/debayer_parity.rs` rather than assumed away.

use rayon::prelude::*;

use crate::bayer::BayerPattern;
use crate::error::ImageProcError;

/// A filter tap: column offset, row offset, weight.
type Tap = (i8, i8, f32);

/// Every kernel in the paper is stated over eighths.
const SCALE: f32 = 1.0 / 8.0;

/// Green at a red or blue site.
///
/// ```text
///  0  0 -1  0  0
///  0  0  2  0  0
/// -1  2  4  2 -1
///  0  0  2  0  0
///  0  0 -1  0  0
/// ```
const GREEN_AT_RB: [Tap; 9] = [
    (0, -2, -1.0),
    (0, -1, 2.0),
    (-2, 0, -1.0),
    (-1, 0, 2.0),
    (0, 0, 4.0),
    (1, 0, 2.0),
    (2, 0, -1.0),
    (0, 1, 2.0),
    (0, 2, -1.0),
];

/// Red at a green site whose **row** carries red (equivalently blue at a green
/// site whose row carries blue).
///
/// ```text
///  0   0  1/2  0   0
///  0  -1   0  -1   0
/// -1   4   5   4  -1
///  0  -1   0  -1   0
///  0   0  1/2  0   0
/// ```
const RB_AT_GREEN_ALONG_ROW: [Tap; 13] = [
    (0, -2, 0.5),
    (-1, -1, -1.0),
    (1, -1, -1.0),
    (-2, 0, -1.0),
    (-1, 0, 4.0),
    (0, 0, 5.0),
    (1, 0, 4.0),
    (2, 0, -1.0),
    (-1, 1, -1.0),
    (1, 1, -1.0),
    (0, 2, 0.5),
    // Padding taps so the array length is fixed; zero weight, no effect.
    (0, 0, 0.0),
    (0, 0, 0.0),
];

/// Red at a green site whose **column** carries red — the transpose of
/// [`RB_AT_GREEN_ALONG_ROW`].
///
/// ```text
///   0   0  -1   0   0
///   0  -1   4  -1   0
///  1/2  0   5   0  1/2
///   0  -1   4  -1   0
///   0   0  -1   0   0
/// ```
const RB_AT_GREEN_ALONG_COLUMN: [Tap; 13] = [
    (0, -2, -1.0),
    (-1, -1, -1.0),
    (0, -1, 4.0),
    (1, -1, -1.0),
    (-2, 0, 0.5),
    (0, 0, 5.0),
    (2, 0, 0.5),
    (-1, 1, -1.0),
    (0, 1, 4.0),
    (1, 1, -1.0),
    (0, 2, -1.0),
    (0, 0, 0.0),
    (0, 0, 0.0),
];

/// Red at a blue site, or blue at a red site.
///
/// ```text
///   0    0  -3/2  0    0
///   0    2   0    2    0
/// -3/2   0   6    0  -3/2
///   0    2   0    2    0
///   0    0  -3/2  0    0
/// ```
const RB_AT_OPPOSITE: [Tap; 9] = [
    (0, -2, -1.5),
    (-1, -1, 2.0),
    (1, -1, 2.0),
    (-2, 0, -1.5),
    (0, 0, 6.0),
    (2, 0, -1.5),
    (-1, 1, 2.0),
    (1, 1, 2.0),
    (0, 2, -1.5),
];

/// Colour a CFA site carries: 0 red, 1 green, 2 blue.
#[inline]
fn colour_at(pattern: BayerPattern, x: usize, y: usize) -> usize {
    let tile = match pattern {
        BayerPattern::Rggb => [[0u8, 1], [1, 2]],
        BayerPattern::Bggr => [[2, 1], [1, 0]],
        BayerPattern::Gbrg => [[1, 2], [0, 1]],
        BayerPattern::Grbg => [[1, 0], [2, 1]],
        BayerPattern::None => [[1, 1], [1, 1]],
    };
    tile[y % 2][x % 2] as usize
}

#[inline]
fn apply(cfa: &[f32], width: usize, x: usize, y: usize, taps: &[Tap]) -> f32 {
    let mut sum = 0.0f32;
    for &(dx, dy, weight) in taps {
        if weight == 0.0 {
            continue;
        }
        let idx = (y as isize + dy as isize) as usize * width + (x as isize + dx as isize) as usize;
        sum += weight * cfa[idx];
    }
    sum * SCALE
}

/// Demosaics a single-channel CFA frame into planar RGB at full resolution.
///
/// Returns `width * height * 3` samples: all of red, then green, then blue —
/// the layout `Frame` uses.
pub fn demosaic(
    cfa: &[f32],
    width: usize,
    height: usize,
    pattern: BayerPattern,
) -> Result<Vec<f32>, ImageProcError> {
    if pattern == BayerPattern::None {
        return Err(ImageProcError::Processing(
            "demosaicing needs a Bayer pattern".to_string(),
        ));
    }
    if cfa.len() != width * height {
        return Err(ImageProcError::Processing(format!(
            "CFA buffer has {} samples, expected {}",
            cfa.len(),
            width * height
        )));
    }
    // The filters reach two pixels out. Smaller than that and there is no
    // interior at all, which is not an image worth demosaicing.
    if width < 5 || height < 5 {
        return Err(ImageProcError::Processing(format!(
            "frame is {width}x{height}; demosaicing needs at least 5x5"
        )));
    }

    let plane = width * height;
    let mut out = vec![0.0f32; plane * 3];
    let (red, rest) = out.split_at_mut(plane);
    let (green, blue) = rest.split_at_mut(plane);

    // Rows are independent; each reads two rows either side.
    red.par_chunks_mut(width)
        .zip(green.par_chunks_mut(width))
        .zip(blue.par_chunks_mut(width))
        .enumerate()
        .for_each(|(y, ((r_row, g_row), b_row))| {
            for x in 0..width {
                let (r, g, b) = if x < 2 || y < 2 || x + 2 >= width || y + 2 >= height {
                    // No room for the 5x5 support. Bilinear there costs a
                    // two-pixel frame of slightly softer edges and avoids
                    // inventing data by mirroring, which on a star field would
                    // duplicate sources.
                    border_pixel(cfa, width, height, pattern, x, y)
                } else {
                    interior_pixel(cfa, width, pattern, x, y)
                };
                r_row[x] = r;
                g_row[x] = g;
                b_row[x] = b;
            }
        });

    Ok(out)
}

/// Reconstructs one interior pixel, where the full 5×5 support is available.
#[inline]
fn interior_pixel(
    cfa: &[f32],
    width: usize,
    pattern: BayerPattern,
    x: usize,
    y: usize,
) -> (f32, f32, f32) {
    let centre = cfa[y * width + x];

    match colour_at(pattern, x, y) {
        // Red site: red measured, green from the cross filter, blue from the
        // opposite-colour filter.
        0 => (
            centre,
            apply(cfa, width, x, y, &GREEN_AT_RB),
            apply(cfa, width, x, y, &RB_AT_OPPOSITE),
        ),
        // Blue site: the mirror image.
        2 => (
            apply(cfa, width, x, y, &RB_AT_OPPOSITE),
            apply(cfa, width, x, y, &GREEN_AT_RB),
            centre,
        ),
        // Green site. One axis through this pixel carries red and the other
        // blue; which is which decides the filter each colour gets.
        _ => {
            let red_is_along_row = colour_at(pattern, x + 1, y) == 0;
            let (r, b) = if red_is_along_row {
                (
                    apply(cfa, width, x, y, &RB_AT_GREEN_ALONG_ROW),
                    apply(cfa, width, x, y, &RB_AT_GREEN_ALONG_COLUMN),
                )
            } else {
                (
                    apply(cfa, width, x, y, &RB_AT_GREEN_ALONG_COLUMN),
                    apply(cfa, width, x, y, &RB_AT_GREEN_ALONG_ROW),
                )
            };
            (r, centre, b)
        }
    }
}

/// Bilinear reconstruction for the two-pixel border, where the 5×5 support
/// runs off the frame.
fn border_pixel(
    cfa: &[f32],
    width: usize,
    height: usize,
    pattern: BayerPattern,
    x: usize,
    y: usize,
) -> (f32, f32, f32) {
    let mut sums = [0.0f32; 3];
    let mut counts = [0usize; 3];
    for dy in -1isize..=1 {
        for dx in -1isize..=1 {
            let (nx, ny) = (x as isize + dx, y as isize + dy);
            if nx < 0 || ny < 0 || nx >= width as isize || ny >= height as isize {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            let c = colour_at(pattern, nx, ny);
            sums[c] += cfa[ny * width + nx];
            counts[c] += 1;
        }
    }

    let mean = |c: usize| {
        if counts[c] == 0 {
            0.0
        } else {
            sums[c] / counts[c] as f32
        }
    };

    let measured = colour_at(pattern, x, y);
    let mut rgb = [mean(0), mean(1), mean(2)];
    // The measured channel is known exactly; never average it away.
    rgb[measured] = cfa[y * width + x];
    (rgb[0], rgb[1], rgb[2])
}
