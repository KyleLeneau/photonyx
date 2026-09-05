//! ADR 006 P9-T9: visual regression against `astroimage`'s preview output.
//!
//! `astroimage`'s hand-tuned SIMD kernels and this crate's safe scalar/rayon
//! ports round floating-point sums in different orders, so exact byte
//! equality isn't the bar — a small per-pixel tolerance is. What must hold
//! bit-for-bit is shape: same output dimensions, same channel count.

use std::path::Path;

use px_imageproc::decode_preview;

#[path = "support.rs"]
mod support;

/// Max allowed per-channel-byte difference. STF autostretch is a division by
/// a difference of two f32 sums (median, MAD) computed via quickselect over
/// possibly differently-ordered samples; a few sample-order-dependent ULPs
/// going into that division can move the u8 output by more than one, so this
/// is deliberately looser than "off by rounding."
const TOLERANCE: i32 = 3;

/// Fraction of pixels allowed to exceed `TOLERANCE` before the test fails —
/// covers the odd pixel landing exactly on a stretch curve's steep part,
/// without licensing systematic drift.
const MAX_OUTLIER_FRACTION: f64 = 0.001;

fn assert_previews_match(path: &Path) {
    let ours = decode_preview(path).expect("px-imageproc decode_preview");
    let (oracle_w, oracle_h, oracle_pixels) = support::astroimage_oracle_decode_preview(path);

    assert_eq!(
        (ours.width, ours.height),
        (oracle_w, oracle_h),
        "output dimensions must match exactly"
    );
    assert_eq!(
        ours.pixels.len(),
        oracle_pixels.len(),
        "byte length must match"
    );

    let mut outliers = 0usize;
    let mut max_diff = 0i32;
    for (a, b) in ours.pixels.iter().zip(oracle_pixels.iter()) {
        let diff = (*a as i32 - *b as i32).abs();
        max_diff = max_diff.max(diff);
        if diff > TOLERANCE {
            outliers += 1;
        }
    }

    let outlier_fraction = outliers as f64 / ours.pixels.len() as f64;
    assert!(
        outlier_fraction <= MAX_OUTLIER_FRACTION,
        "{outliers}/{} bytes ({:.4}%) exceeded tolerance {TOLERANCE} (max diff seen: {max_diff})",
        ours.pixels.len(),
        outlier_fraction * 100.0,
    );
}

#[test]
fn bayer_rggb_matches_astroimage() {
    let dir = support::scratch_dir();
    let path = support::write_synthetic_image(&dir, 512, 512, Some("RGGB"));
    assert_previews_match(&path);
}

#[test]
fn bayer_bggr_matches_astroimage() {
    let dir = support::scratch_dir();
    let path = support::write_synthetic_image(&dir, 512, 512, Some("BGGR"));
    assert_previews_match(&path);
}

#[test]
fn mono_matches_astroimage() {
    let dir = support::scratch_dir();
    let path = support::write_synthetic_image(&dir, 512, 512, None);
    assert_previews_match(&path);
}

#[test]
fn odd_dimensions_match_astroimage() {
    // Not evenly divisible by the debayer factor's usual powers of two —
    // exercises the truncating-division edges in downscale/debayer.
    let dir = support::scratch_dir();
    let path = support::write_synthetic_image(&dir, 517, 383, Some("GBRG"));
    assert_previews_match(&path);
}

#[test]
fn large_frame_needing_downscale_matches_astroimage() {
    // Exceeds MAX_DISPLAY_DIM, forcing the downscale-on-top-of-debayer path.
    let dir = support::scratch_dir();
    let path = support::write_synthetic_image(&dir, 4096, 4096, Some("RGGB"));
    assert_previews_match(&path);
}
