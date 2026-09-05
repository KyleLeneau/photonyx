//! 2x2 box averaging for mono preview mode (ADR 006, Phase 9, P9-T6).
//!
//! Unlike [`crate::downscale`], this averages each 2x2 block instead of
//! picking one sample — used only for the mono, non-Bayer preview path,
//! where there's no debayer step already discarding 3/4 of the samples.
//! Matches `astroimage`'s `bin_2x2_float` bit-for-bit (P9-T6 parity target).

use rayon::prelude::*;

pub fn bin_2x2(input: &[f32], width: usize, height: usize) -> (Vec<f32>, usize, usize) {
    let out_w = width / 2;
    let out_h = height / 2;
    let mut output = vec![0f32; out_w * out_h];

    output
        .par_chunks_mut(out_w)
        .enumerate()
        .for_each(|(y, out_row)| {
            let row0 = (y * 2) * width;
            let row1 = (y * 2 + 1) * width;
            for (x, dst) in out_row.iter_mut().enumerate() {
                let in_x = x * 2;
                *dst = (input[row0 + in_x]
                    + input[row0 + in_x + 1]
                    + input[row1 + in_x]
                    + input[row1 + in_x + 1])
                    * 0.25;
            }
        });

    (output, out_w, out_h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn averages_each_2x2_block() {
        #[rustfmt::skip]
        let input: Vec<f32> = vec![
            1.0, 2.0, 3.0, 4.0,
            5.0, 6.0, 7.0, 8.0,
            9.0, 10.0, 11.0, 12.0,
            13.0, 14.0, 15.0, 16.0,
        ];
        let (out, w, h) = bin_2x2(&input, 4, 4);
        assert_eq!((w, h), (2, 2));
        // Block [1,2,5,6] -> 3.5; [3,4,7,8] -> 5.5;
        // [9,10,13,14] -> 11.5; [11,12,15,16] -> 13.5
        assert_eq!(out, vec![3.5, 5.5, 11.5, 13.5]);
    }
}
