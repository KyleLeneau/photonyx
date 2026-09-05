//! Integer nearest-neighbor downscale (ADR 006, Phase 9, P9-T6).
//!
//! Preview-only: throwing away samples is fine when the output is going to
//! be shown at a fraction of native resolution anyway, and it's the cheapest
//! way to respect [`crate::MAX_DISPLAY_DIM`] without allocating an
//! intermediate at native size. Matches `astroimage`'s
//! `downscale_f32_planar` bit-for-bit (P9-T6 parity target).

use rayon::prelude::*;

/// Downscales planar multi-channel f32 data (each channel stored
/// contiguously) by picking every `factor`-th sample in each axis.
/// `width`/`height` must be evenly divisible by `factor` for the debayer
/// caller (ADR 006 P9-T6's even-factor constraint); other callers get
/// `width/factor` truncated.
pub fn downscale_planar(
    input: &[f32],
    width: usize,
    height: usize,
    channels: usize,
    factor: usize,
) -> (Vec<f32>, usize, usize) {
    debug_assert!(factor >= 1);
    let new_w = width / factor;
    let new_h = height / factor;
    let plane_in = width * height;
    let plane_out = new_w * new_h;
    let mut output = vec![0f32; plane_out * channels];

    output
        .par_chunks_mut(plane_out)
        .enumerate()
        .for_each(|(c, dst_plane)| {
            let src = &input[c * plane_in..];
            dst_plane
                .par_chunks_mut(new_w)
                .enumerate()
                .for_each(|(y, row)| {
                    let src_row = &src[y * factor * width..];
                    for (x, dst) in row.iter_mut().enumerate() {
                        *dst = src_row[x * factor];
                    }
                });
        });

    (output, new_w, new_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factor_1_is_identity() {
        let input = vec![1.0, 2.0, 3.0, 4.0];
        let (out, w, h) = downscale_planar(&input, 2, 2, 1, 1);
        assert_eq!((w, h), (2, 2));
        assert_eq!(out, input);
    }

    #[test]
    fn factor_2_picks_top_left_of_each_block() {
        #[rustfmt::skip]
        let input: Vec<f32> = vec![
            1.0, 2.0, 3.0, 4.0,
            5.0, 6.0, 7.0, 8.0,
            9.0, 10.0, 11.0, 12.0,
            13.0, 14.0, 15.0, 16.0,
        ];
        let (out, w, h) = downscale_planar(&input, 4, 4, 1, 2);
        assert_eq!((w, h), (2, 2));
        assert_eq!(out, vec![1.0, 3.0, 9.0, 11.0]);
    }

    #[test]
    fn planar_channels_are_independent() {
        // 2 channels, each 2x2, factor 2 -> each channel collapses to 1 pixel.
        let ch0 = [1.0, 2.0, 3.0, 4.0];
        let ch1 = [10.0, 20.0, 30.0, 40.0];
        let input: Vec<f32> = ch0.iter().chain(ch1.iter()).copied().collect();
        let (out, w, h) = downscale_planar(&input, 2, 2, 2, 2);
        assert_eq!((w, h), (1, 1));
        assert_eq!(out, vec![1.0, 10.0]);
    }
}
