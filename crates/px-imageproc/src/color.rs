//! Small pixel-format helpers shared by the preview pipeline (ADR 006,
//! Phase 9).

/// Replicates a grayscale u8 plane to interleaved RGB (`gray[i]` becomes
/// `(gray[i], gray[i], gray[i])`).
pub fn replicate_gray_to_rgb(gray: &[u8]) -> Vec<u8> {
    let mut rgb = vec![0u8; gray.len() * 3];
    for (i, &val) in gray.iter().enumerate() {
        rgb[i * 3] = val;
        rgb[i * 3 + 1] = val;
        rgb[i * 3 + 2] = val;
    }
    rgb
}

/// Flips an image with `bytes_per_pixel` bytes per pixel top-to-bottom, in
/// place. Used for `ROWORDER = 'TOP-DOWN'` FITS data, which stores rows in
/// screen order rather than the FITS-standard bottom-up order.
pub fn vertical_flip(data: &mut [u8], width: usize, height: usize, bytes_per_pixel: usize) {
    let row_bytes = width * bytes_per_pixel;
    let mut temp = vec![0u8; row_bytes];
    for y in 0..height / 2 {
        let top = y * row_bytes;
        let bot = (height - 1 - y) * row_bytes;
        temp.copy_from_slice(&data[top..top + row_bytes]);
        data.copy_within(bot..bot + row_bytes, top);
        data[bot..bot + row_bytes].copy_from_slice(&temp);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replicates_each_gray_value_three_times() {
        let gray = [10u8, 20, 30];
        assert_eq!(
            replicate_gray_to_rgb(&gray),
            vec![10, 10, 10, 20, 20, 20, 30, 30, 30]
        );
    }

    #[test]
    fn flip_reverses_row_order() {
        // 2x3 image, 1 byte per pixel: rows [1,1], [2,2], [3,3].
        let mut data = vec![1, 1, 2, 2, 3, 3];
        vertical_flip(&mut data, 2, 3, 1);
        assert_eq!(data, vec![3, 3, 2, 2, 1, 1]);
    }

    #[test]
    fn flip_is_involution() {
        let original = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        let mut data = original.clone();
        vertical_flip(&mut data, 2, 4, 1);
        vertical_flip(&mut data, 2, 4, 1);
        assert_eq!(data, original);
    }
}
