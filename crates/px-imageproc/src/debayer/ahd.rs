//! Adaptive homogeneity-directed demosaicing (ADR 007, Phase 6b).
//!
//! Implements the method of Hirakawa and Parks, *Adaptive
//! Homogeneity-Directed Demosaicing Algorithm*, IEEE Transactions on Image
//! Processing 14(3):360–369, 2005. The paper states the method in full, which
//! is what makes it implementable from its published description.
//!
//! The premise is that a demosaic goes wrong when it interpolates *across* an
//! edge rather than along it, and that the interpolation itself cannot tell
//! which it did. So AHD does not choose a direction up front. It reconstructs
//! the whole frame twice — once interpolating only horizontally, once only
//! vertically — converts both candidates to CIELAB, and then asks, per pixel,
//! which candidate landed in the more *homogeneous* neighbourhood: how many of
//! its four neighbours are close to it in both luminance and chrominance. An
//! interpolation that crossed an edge leaves a neighbourhood that disagrees
//! with itself; one that ran along the edge does not. The more homogeneous
//! candidate wins.
//!
//! The four stages below follow the paper's:
//!
//! 1. Directional green, by the five-tap filter `[-1/4, 1/2, 1/2, 1/2, -1/4]`
//!    applied along one axis of the CFA. On a red row that reads
//!    `(G₋₁ + G₊₁)/2 + (2R₀ − R₋₂ − R₊₂)/4` — a bilinear term plus a
//!    second-derivative correction taken from the measured channel.
//! 2. Red and blue for each candidate, by bilinear interpolation of the
//!    colour *differences* R−G and B−G against that candidate's green. Colour
//!    differences are far flatter than the channels themselves, which is why
//!    interpolating them rather than the channels is what the literature does.
//! 3. CIELAB for both candidates. The frame is linear, so it goes to XYZ
//!    through the Rec. 709 matrix with no gamma step — applying an sRGB
//!    transfer curve to linear sensor data would be wrong here.
//! 4. Homogeneity maps with per-pixel adaptive thresholds, summed over a 3×3
//!    window, and the larger wins. An exact tie averages the two, which is the
//!    honest answer when the map has no preference.
//!
//! **What is deliberately omitted.** The paper closes with an optional
//! artifact-reduction pass: a few iterations of 3×3 median filtering on the
//! R−G and B−G difference planes. That is designed for natural images, where
//! an isolated pixel that disagrees with its neighbours in colour is an
//! artifact. On a star field an isolated pixel that disagrees with its
//! neighbours in colour is frequently a star, and a median filter removes it.
//! The core selection is what Phase 6b is measuring; the median pass is not
//! implemented rather than implemented and disabled.
//!
//! **This is still not RCD**, the method Siril uses by default; RCD has no
//! published specification independent of its GPL implementation. AHD is the
//! closest published relative — both are adaptive, both avoid interpolating
//! across an edge — and the gap between them is measured in
//! `tests/debayer_parity.rs` rather than assumed away.

use rayon::prelude::*;

use crate::bayer::BayerPattern;
use crate::error::ImageProcError;

/// How far the pipeline reaches, in pixels, to produce one output sample.
///
/// The 3×3 homogeneity window reads homogeneity at ±1, which reads CIELAB at
/// ±2, which is the candidate RGB at ±2, which reads directional green at ±3,
/// which reads the CFA at ±5. Inside that margin of the frame edge there is no
/// support, and bilinear fills it — the same fallback [`super::mhc`] uses, and
/// for the same reason: mirroring would invent data, which on a star field
/// means duplicating sources.
const MARGIN: usize = 5;

/// Rows are processed in bands so the eight intermediate planes each band
/// needs are sized by this rather than by the frame. A 61 MP frame would want
/// close to 4 GB of intermediates if they were held whole; at this size a band
/// costs ~15 MB, and bands run in parallel.
const BAND_PIXELS: usize = 262_144;

/// Diagonal neighbours: where the opposite chroma sits, seen from a red or
/// blue site.
const DIAGONALS: [(i32, i32); 4] = [(-1, -1), (1, -1), (-1, 1), (1, 1)];
const HORIZONTAL: [(i32, i32); 2] = [(-1, 0), (1, 0)];
const VERTICAL: [(i32, i32); 2] = [(0, -1), (0, 1)];

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
    let min_side = 2 * MARGIN + 1;
    if width < min_side || height < min_side {
        return Err(ImageProcError::Processing(format!(
            "frame is {width}x{height}; demosaicing needs at least {min_side}x{min_side}"
        )));
    }

    // The border comes from bilinear, and so does every pixel the interior
    // pass then overwrites. Running it over the whole frame first costs one
    // cheap pass and keeps the border handling in one place instead of
    // threaded through four stages.
    let mut out = super::bilinear::demosaic(cfa, width, height, pattern)?;

    let plane = width * height;
    let band_rows = (BAND_PIXELS / width).clamp(8, 256);
    let interior = MARGIN * width..(height - MARGIN) * width;

    let (red, rest) = out.split_at_mut(plane);
    let (green, blue) = rest.split_at_mut(plane);

    red[interior.clone()]
        .par_chunks_mut(band_rows * width)
        .zip(green[interior.clone()].par_chunks_mut(band_rows * width))
        .zip(blue[interior].par_chunks_mut(band_rows * width))
        .enumerate()
        .for_each(|(band, ((r_rows, g_rows), b_rows))| {
            let y0 = MARGIN + band * band_rows;
            let y1 = y0 + r_rows.len() / width;
            demosaic_band(cfa, width, height, pattern, y0, y1, r_rows, g_rows, b_rows);
        });

    Ok(out)
}

/// Reconstructs output rows `y0..y1`, reading `MARGIN` rows of context either
/// side.
#[allow(clippy::too_many_arguments)]
fn demosaic_band(
    cfa: &[f32],
    width: usize,
    height: usize,
    pattern: BayerPattern,
    y0: usize,
    y1: usize,
    r_rows: &mut [f32],
    g_rows: &mut [f32],
    b_rows: &mut [f32],
) {
    let top = y0 - MARGIN;
    let bot = (y1 + MARGIN).min(height);
    let rows = bot - top;
    let band = width * rows;

    let (green_h, green_v) = directional_green(cfa, width, height, pattern, top, bot);
    let rgb_h = colour_from_green(cfa, &green_h, width, height, pattern, top, bot);
    let rgb_v = colour_from_green(cfa, &green_v, width, height, pattern, top, bot);
    drop(green_h);
    drop(green_v);

    let lab_h = to_lab(&rgb_h, band);
    let lab_v = to_lab(&rgb_v, band);
    let (homo_h, homo_v) = homogeneity(&lab_h, &lab_v, width, rows);
    drop(lab_h);
    drop(lab_v);

    for y in y0..y1 {
        let local = y - top;
        let out_row = (y - y0) * width;
        for x in MARGIN..width - MARGIN {
            // Sum each direction's homogeneity over a 3x3 window. A single
            // pixel's count is 0..=4 and too coarse to decide on; the window
            // is what the paper selects on.
            let (mut sum_h, mut sum_v) = (0u32, 0u32);
            for dy in local - 1..=local + 1 {
                for dx in x - 1..=x + 1 {
                    sum_h += u32::from(homo_h[dy * width + dx]);
                    sum_v += u32::from(homo_v[dy * width + dx]);
                }
            }

            let i = local * width + x;
            let pick = |c: usize| {
                let (h, v) = (rgb_h[c * band + i], rgb_v[c * band + i]);
                match sum_h.cmp(&sum_v) {
                    std::cmp::Ordering::Greater => h,
                    std::cmp::Ordering::Less => v,
                    // Neither neighbourhood is more homogeneous than the
                    // other, so there is no directional evidence to act on.
                    std::cmp::Ordering::Equal => 0.5 * (h + v),
                }
            };
            r_rows[out_row + x] = pick(0);
            g_rows[out_row + x] = pick(1);
            b_rows[out_row + x] = pick(2);
        }
    }
}

/// Green over rows `top..bot`, interpolated horizontally and vertically.
///
/// Returns two `width * (bot - top)` planes. At a green site both carry the
/// measured value; elsewhere each carries the five-tap estimate along its own
/// axis, or — where that filter runs off the frame — the mean of the adjacent
/// measured greens.
fn directional_green(
    cfa: &[f32],
    width: usize,
    height: usize,
    pattern: BayerPattern,
    top: usize,
    bot: usize,
) -> (Vec<f32>, Vec<f32>) {
    let mut green_h = vec![0.0f32; width * (bot - top)];
    let mut green_v = vec![0.0f32; width * (bot - top)];

    for y in top..bot {
        let row = (y - top) * width;
        for x in 0..width {
            let g = y * width + x;
            let centre = cfa[g];
            if pattern.colour_at(x, y) == 1 {
                green_h[row + x] = centre;
                green_v[row + x] = centre;
                continue;
            }

            green_h[row + x] = if x >= 2 && x + 2 < width {
                0.5 * (cfa[g - 1] + cfa[g + 1]) + 0.25 * (2.0 * centre - cfa[g - 2] - cfa[g + 2])
            } else {
                mean_of(cfa, width, height, x, y, &HORIZONTAL).unwrap_or(centre)
            };
            green_v[row + x] = if y >= 2 && y + 2 < height {
                0.5 * (cfa[g - width] + cfa[g + width])
                    + 0.25 * (2.0 * centre - cfa[g - 2 * width] - cfa[g + 2 * width])
            } else {
                mean_of(cfa, width, height, x, y, &VERTICAL).unwrap_or(centre)
            };
        }
    }

    (green_h, green_v)
}

/// One candidate's RGB over rows `top..bot`, as three planes of
/// `width * (bot - top)`.
///
/// Green is taken as given. Red and blue come from bilinear interpolation of
/// R−G and B−G against it, evaluated over whichever neighbours carry the
/// colour being reconstructed.
fn colour_from_green(
    cfa: &[f32],
    green: &[f32],
    width: usize,
    height: usize,
    pattern: BayerPattern,
    top: usize,
    bot: usize,
) -> Vec<f32> {
    let band = width * (bot - top);
    let mut rgb = vec![0.0f32; band * 3];

    for y in top..bot {
        let row = (y - top) * width;
        for x in 0..width {
            let i = row + x;
            let centre = cfa[y * width + x];
            let g = green[i];

            let (r, b) = match pattern.colour_at(x, y) {
                // A red site measures red; blue lives on the diagonals.
                0 => (
                    centre,
                    g + diff_mean(cfa, green, width, height, top, bot, x, y, &DIAGONALS),
                ),
                2 => (
                    g + diff_mean(cfa, green, width, height, top, bot, x, y, &DIAGONALS),
                    centre,
                ),
                // A green site has red along one axis and blue along the
                // other. `x ^ 1` is the horizontal partner without running off
                // the row, and the tile is 2-periodic so it names the same
                // colour as `x + 1`.
                _ => {
                    let (red_axis, blue_axis) = if pattern.colour_at(x ^ 1, y) == 0 {
                        (&HORIZONTAL, &VERTICAL)
                    } else {
                        (&VERTICAL, &HORIZONTAL)
                    };
                    (
                        g + diff_mean(cfa, green, width, height, top, bot, x, y, red_axis),
                        g + diff_mean(cfa, green, width, height, top, bot, x, y, blue_axis),
                    )
                }
            };

            rgb[i] = r;
            rgb[band + i] = g;
            rgb[2 * band + i] = b;
        }
    }

    rgb
}

/// Mean of the colour difference `cfa - green` over the offsets that lie
/// inside both the frame and the band. Zero when none do, which leaves the
/// reconstructed channel equal to green — a grey pixel, and the only answer
/// available with no chroma neighbour in reach.
#[allow(clippy::too_many_arguments)]
#[inline]
fn diff_mean(
    cfa: &[f32],
    green: &[f32],
    width: usize,
    height: usize,
    top: usize,
    bot: usize,
    x: usize,
    y: usize,
    offsets: &[(i32, i32)],
) -> f32 {
    let mut sum = 0.0f32;
    let mut count = 0usize;
    for &(dx, dy) in offsets {
        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
        if nx < 0 || ny < 0 || nx as usize >= width || ny as usize >= height {
            continue;
        }
        let (nx, ny) = (nx as usize, ny as usize);
        if ny < top || ny >= bot {
            continue;
        }
        sum += cfa[ny * width + nx] - green[(ny - top) * width + nx];
        count += 1;
    }
    if count == 0 { 0.0 } else { sum / count as f32 }
}

/// Mean of the CFA over the offsets that lie inside the frame, or `None` if
/// none do.
#[inline]
fn mean_of(
    cfa: &[f32],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    offsets: &[(i32, i32)],
) -> Option<f32> {
    let mut sum = 0.0f32;
    let mut count = 0usize;
    for &(dx, dy) in offsets {
        let (nx, ny) = (x as i32 + dx, y as i32 + dy);
        if nx < 0 || ny < 0 || nx as usize >= width || ny as usize >= height {
            continue;
        }
        sum += cfa[ny as usize * width + nx as usize];
        count += 1;
    }
    (count > 0).then(|| sum / count as f32)
}

/// Converts a planar linear-RGB band to planar CIELAB.
///
/// No gamma step: the frame is linear sensor data, and the Rec. 709 matrix
/// expects exactly that. `L*` lands on its usual 0..100 for values in the
/// [0, 1] convention the rest of the crate uses.
fn to_lab(rgb: &[f32], band: usize) -> Vec<f32> {
    let mut lab = vec![0.0f32; band * 3];
    for i in 0..band {
        let (r, g, b) = (rgb[i], rgb[band + i], rgb[2 * band + i]);

        // Rec. 709 primaries, D65 white, each axis normalized by the white
        // point so the f() below sees Y/Yn rather than Y.
        let fx = lab_f((0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b) / 0.950_47);
        let fy = lab_f(0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b);
        let fz = lab_f((0.019_333_9 * r + 0.119_192 * g + 0.950_304_1 * b) / 1.088_83);

        lab[i] = 116.0 * fy - 16.0;
        lab[band + i] = 500.0 * (fx - fy);
        lab[2 * band + i] = 200.0 * (fy - fz);
    }
    lab
}

/// CIELAB's nonlinearity, with the linear segment near zero that keeps its
/// slope finite there.
///
/// Calibrated astro frames genuinely contain negative samples — a hot pixel in
/// the master dark subtracted from an ordinary light — and the cube root is
/// undefined for those. They are clamped to zero, which costs nothing the
/// homogeneity test can use: below the black point there is no colour to
/// compare.
#[inline]
fn lab_f(t: f32) -> f32 {
    const DELTA: f32 = 6.0 / 29.0;
    let t = t.max(0.0);
    if t > DELTA * DELTA * DELTA {
        t.cbrt()
    } else {
        t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
    }
}

/// Counts, for each candidate and each pixel, how many of its four neighbours
/// are close to it in both luminance and chrominance — the paper's homogeneity
/// map, on the δ = 1 ball.
///
/// The thresholds are per pixel and adaptive, and the way they are formed is
/// the part that makes the test meaningful. Each is the *smaller* of what the
/// two candidates see along their own axis. Taking the smaller means the
/// threshold is set by whichever direction is locally smoother, so a candidate
/// that interpolated across an edge cannot pass by declaring its own large
/// differences to be the local norm.
fn homogeneity(lab_h: &[f32], lab_v: &[f32], width: usize, rows: usize) -> (Vec<u8>, Vec<u8>) {
    let band = width * rows;
    let mut homo_h = vec![0u8; band];
    let mut homo_v = vec![0u8; band];

    // Luminance distance and squared chrominance distance between two pixels
    // of one candidate.
    let luma = |lab: &[f32], a: usize, b: usize| (lab[a] - lab[b]).abs();
    let chroma = |lab: &[f32], a: usize, b: usize| {
        let da = lab[band + a] - lab[band + b];
        let db = lab[2 * band + a] - lab[2 * band + b];
        da * da + db * db
    };

    for y in 1..rows - 1 {
        for x in 1..width - 1 {
            let i = y * width + x;
            let (left, right) = (i - 1, i + 1);
            let (up, down) = (i - width, i + width);

            let eps_l = luma(lab_h, i, left)
                .max(luma(lab_h, i, right))
                .min(luma(lab_v, i, up).max(luma(lab_v, i, down)));
            let eps_c = chroma(lab_h, i, left)
                .max(chroma(lab_h, i, right))
                .min(chroma(lab_v, i, up).max(chroma(lab_v, i, down)));

            let count = |lab: &[f32]| {
                [left, right, up, down]
                    .iter()
                    .filter(|&&n| luma(lab, i, n) <= eps_l && chroma(lab, i, n) <= eps_c)
                    .count() as u8
            };
            homo_h[i] = count(lab_h);
            homo_v[i] = count(lab_v);
        }
    }

    (homo_h, homo_v)
}
