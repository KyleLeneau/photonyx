//! Shared test-only fixture generation (mirrors `px-fits/tests/support.rs`
//! and `benches/support.rs`'s pattern).

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use px_fits::card::Value;
use px_fits::{BitPix, FitsWriter, HeaderBuilder};

struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_i16(&mut self) -> i16 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        (z ^ (z >> 31)) as i16
    }
}

pub fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join("px-imageproc-test-fixtures")
}

/// Writes a deterministic BITPIX=16 (signed) synthetic FITS image, optionally
/// tagged with a `BAYERPAT` card. Data has a realistic dim-background/
/// bright-signal split rather than pure noise, so autostretch produces a
/// non-degenerate result.
pub fn write_synthetic_image(
    dir: &Path,
    width: usize,
    height: usize,
    bayer_pat: Option<&str>,
) -> PathBuf {
    std::fs::create_dir_all(dir).expect("create test temp dir");
    let name = match bayer_pat {
        Some(p) => format!("synthetic_{width}x{height}_i16_{p}.fits"),
        None => format!("synthetic_{width}x{height}_i16.fits"),
    };
    let path = dir.join(name);
    if path.exists() {
        return path;
    }

    let mut header = HeaderBuilder::primary_image(BitPix::I16, &[width as u64, height as u64])
        .expect("build header");
    if let Some(pat) = bayer_pat {
        header = header
            .card("BAYERPAT", Value::String(pat.to_string()), None)
            .expect("BAYERPAT card");
    }

    let mut rng = SplitMix64::new(0xB5_C4_11_E7_00);
    let data: Vec<i16> = (0..width * height)
        .map(|i| {
            // A faint background (~500 ADU) with a sparse bright "star"
            // field, plus a little dither, so the STF autostretch has a
            // realistic median/MAD to work with instead of a flat frame.
            let dither = (rng.next_i16() % 16) as i32;
            let base = if i % 97 == 0 { 20_000 } else { 500 };
            (base + dither).clamp(0, i16::MAX as i32) as i16
        })
        .collect();

    let mut w = FitsWriter::create(&path).expect("create synthetic fixture");
    w.write_image(&header, &data).expect("write image");
    w.finish().expect("finish");
    path
}

/// The `astroimage`-backed preview pipeline `px-imageproc` replaces (P9-T9's
/// oracle). A direct copy of the pre-Phase-9 `px_fits::display::decode_preview`
/// body, kept here as a dev-dependency-only comparison rather than in any
/// shipped crate.
pub fn astroimage_oracle_decode_preview(path: &Path) -> (usize, usize, Vec<u8>) {
    use astroimage::BayerPattern;
    use astroimage::ImageConverter;

    const MAX_DISPLAY_DIM: usize = 2048;

    let (meta, pixels) = ImageConverter::read_raw(path).expect("astroimage read_raw");
    let is_bayer = meta.bayer_pattern != BayerPattern::None;
    let max_dim = meta.width.max(meta.height);

    let factor = {
        let f = ((max_dim as f64 / MAX_DISPLAY_DIM as f64).ceil() as usize).max(1);
        if is_bayer {
            let even = if f.is_multiple_of(2) { f } else { f + 1 };
            even.max(2)
        } else {
            f
        }
    };

    let image = ImageConverter::new()
        .with_downscale(factor)
        .with_preview_mode()
        .process_data(meta, pixels)
        .expect("astroimage process_data");

    (image.width, image.height, image.data)
}
