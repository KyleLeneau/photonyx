//! Super-pixel (2x2 box) debayer (ADR 006, Phase 9, P9-T3).
//!
//! Averages each 2x2 CFA tile into one RGB pixel: R and B are taken as-is,
//! G is the mean of the tile's two green samples. This halves both
//! dimensions but needs no edge handling or interpolation kernel, which is
//! what makes it cheap enough for an interactive preview path. It matches
//! `astroimage`'s `super_pixel_debayer_*` bit-for-bit (P9-T6 parity target).
//! Output is planar (all R, then all G, then all B) at `width/2 x height/2`.

use rayon::prelude::*;

use crate::bayer::BayerPattern;

/// Debayer planar-output row layout: for each 2x2 input tile `[p00, p01,
/// p10, p11]`, which two indices are R/G/G/B is fixed by the pattern.
fn tile_indices(pattern: BayerPattern) -> (usize, usize, usize, usize) {
    match pattern {
        // (r, ga, gb, b): ga/gb are the two green positions; g = (ga+gb)/2.
        BayerPattern::Rggb => (0, 1, 2, 3),
        BayerPattern::Bggr => (3, 1, 2, 0),
        BayerPattern::Gbrg => (2, 0, 3, 1),
        BayerPattern::Grbg => (1, 0, 3, 2),
        BayerPattern::None => (0, 0, 0, 0),
    }
}

/// Super-pixel debayer of full-resolution CFA data into planar RGB at half
/// resolution. `pattern` must not be [`BayerPattern::None`] — callers check
/// for a Bayer pattern before debayering; this function does not special-case
/// mono input.
pub fn super_pixel_debayer(
    input: &[f32],
    width: usize,
    height: usize,
    pattern: BayerPattern,
) -> (Vec<f32>, usize, usize) {
    super_pixel_debayer_with(input, width, height, pattern, |v| v)
}

/// Same algorithm, reading directly from `u16` samples so the common case
/// (16-bit-unsigned Bayer raw, the overwhelming majority of consumer astro
/// cameras) skips materializing a full-resolution `f32` copy before
/// debayering — `px_fits::ImageHdu::read_full::<u16>()` already applies
/// `BSCALE`/`BZERO`, so this is exact, not an approximation of the `f32`
/// path.
pub fn super_pixel_debayer_u16(
    input: &[u16],
    width: usize,
    height: usize,
    pattern: BayerPattern,
) -> (Vec<f32>, usize, usize) {
    super_pixel_debayer_with(input, width, height, pattern, |v| v as f32)
}

fn super_pixel_debayer_with<S: Copy + Sync>(
    input: &[S],
    width: usize,
    height: usize,
    pattern: BayerPattern,
    to_f32: impl Fn(S) -> f32 + Sync,
) -> (Vec<f32>, usize, usize) {
    let out_w = width / 2;
    let out_h = height / 2;
    let plane_size = out_w * out_h;
    let mut output = vec![0f32; plane_size * 3];

    let (r_plane, rest) = output.split_at_mut(plane_size);
    let (g_plane, b_plane) = rest.split_at_mut(plane_size);
    let (ri, gai, gbi, bi) = tile_indices(pattern);

    r_plane
        .par_chunks_mut(out_w)
        .zip(g_plane.par_chunks_mut(out_w))
        .zip(b_plane.par_chunks_mut(out_w))
        .enumerate()
        .for_each(|(y, ((r_row, g_row), b_row))| {
            let in_y = y * 2;
            for x in 0..out_w {
                let in_x = x * 2;
                let p00 = to_f32(input[in_y * width + in_x]);
                let p01 = to_f32(input[in_y * width + in_x + 1]);
                let p10 = to_f32(input[(in_y + 1) * width + in_x]);
                let p11 = to_f32(input[(in_y + 1) * width + in_x + 1]);
                let tile = [p00, p01, p10, p11];

                r_row[x] = tile[ri];
                g_row[x] = (tile[gai] + tile[gbi]) * 0.5;
                b_row[x] = tile[bi];
            }
        });

    (output, out_w, out_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 4x4 RGGB input, four identical 2x2 tiles of known values, so the
    /// debayered 2x2 output should be a constant R/G/B everywhere.
    #[test]
    fn rggb_super_pixel_matches_expected() {
        // R=10, G=(20+30)/2=25, B=40 per tile.
        #[rustfmt::skip]
        let input: Vec<f32> = vec![
            10.0, 20.0, 10.0, 20.0,
            30.0, 40.0, 30.0, 40.0,
            10.0, 20.0, 10.0, 20.0,
            30.0, 40.0, 30.0, 40.0,
        ];
        let (out, w, h) = super_pixel_debayer(&input, 4, 4, BayerPattern::Rggb);
        assert_eq!((w, h), (2, 2));
        let plane = w * h;
        assert!(out[..plane].iter().all(|&v| v == 10.0), "R plane");
        assert!(out[plane..2 * plane].iter().all(|&v| v == 25.0), "G plane");
        assert!(out[2 * plane..].iter().all(|&v| v == 40.0), "B plane");
    }

    #[test]
    fn bggr_swaps_r_and_b_relative_to_rggb() {
        #[rustfmt::skip]
        let input: Vec<f32> = vec![
            10.0, 20.0,
            30.0, 40.0,
        ];
        let (rggb, ..) = super_pixel_debayer(&input, 2, 2, BayerPattern::Rggb);
        let (bggr, ..) = super_pixel_debayer(&input, 2, 2, BayerPattern::Bggr);
        // Planar layout for a 1x1-pixel output is [R, G, B]. R and B swap
        // between the two patterns; G (mean of the two off-diagonal
        // samples) is identical for both.
        assert_eq!(rggb[0], bggr[2]); // rggb R == bggr B
        assert_eq!(rggb[2], bggr[0]); // rggb B == bggr R
        assert_eq!(rggb[1], bggr[1]); // G unchanged
        assert_eq!(rggb[0], input[0]);
        assert_eq!(rggb[2], input[3]);
    }
}
