//! Bilinear demosaicing.
//!
//! Each missing colour is the mean of its nearest neighbours of that colour.
//! The oldest and simplest reconstruction there is, and worth keeping for one
//! specific reason: averaging *reduces* noise, where every sharpening-based
//! method amplifies it. On a frame whose background is noise rather than
//! detail, that trade can go the other way from how it goes in daylight
//! photography.
//!
//! It pays for that with softness and with colour fringing at sharp edges,
//! because it interpolates across them rather than along them.

use rayon::prelude::*;

use crate::bayer::BayerPattern;
use crate::error::ImageProcError;

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

/// Demosaics a single-channel CFA frame into planar RGB at full resolution.
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

    let plane = width * height;
    let mut out = vec![0.0f32; plane * 3];
    let (red, rest) = out.split_at_mut(plane);
    let (green, blue) = rest.split_at_mut(plane);

    red.par_chunks_mut(width)
        .zip(green.par_chunks_mut(width))
        .zip(blue.par_chunks_mut(width))
        .enumerate()
        .for_each(|(y, ((r_row, g_row), b_row))| {
            for x in 0..width {
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

                let mut rgb = [0.0f32; 3];
                for c in 0..3 {
                    rgb[c] = if counts[c] == 0 {
                        0.0
                    } else {
                        sums[c] / counts[c] as f32
                    };
                }
                // The measured channel is known exactly; never average it away.
                rgb[colour_at(pattern, x, y)] = cfa[y * width + x];

                r_row[x] = rgb[0];
                g_row[x] = rgb[1];
                b_row[x] = rgb[2];
            }
        });

    Ok(out)
}
