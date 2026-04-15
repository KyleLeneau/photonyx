//! FITS display utilities: strided read, debayering, and autostretching for visual preview.
//!
//! ## Memory strategy
//!
//! Rather than loading the entire pixel array into an intermediate `Vec<f32>`,
//! this module reads only the rows that contribute to the output image.
//! For a 24 MP 16-bit sensor displayed at ≤ 2048 px, peak memory per frame is
//! roughly 40 MB instead of ~320 MB.
//!
//! ## Pipeline
//!
//! ```text
//! FITS header  →  compute stride
//!    ↓
//! Strided row reads  →  Vec<[f32;3]> at display resolution
//!    ↓
//! Per-channel percentile autostretch  →  Vec<u8> RGB
//! ```

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;

use fitsrs::card::Value;
use fitsrs::hdu::header::Bitpix;
use fitsrs::{Fits, HDU};
use rayon::prelude::*;

use crate::FitsError;

/// Maximum output dimension in either axis. Keeps each decoded frame ≤ ~25 MB.
pub const MAX_DISPLAY_DIM: usize = 2048;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A decoded, display-ready image with interleaved RGB bytes.
pub struct PreviewImage {
    pub width: usize,
    pub height: usize,
    /// Interleaved RGB bytes, length = `width * height * 3`.
    pub pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
pub enum BayerPattern {
    Rggb,
    Bggr,
    Grbg,
    Gbrg,
}

impl BayerPattern {
    fn from_header(s: &str) -> Option<Self> {
        match s.trim() {
            "RGGB" => Some(Self::Rggb),
            "BGGR" => Some(Self::Bggr),
            "GRBG" => Some(Self::Grbg),
            "GBRG" => Some(Self::Gbrg),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Internal header summary
// ---------------------------------------------------------------------------

struct FitsInfo {
    width: usize,
    height: usize,
    is_rgb3d: bool,
    bayer: Option<BayerPattern>,
    bzero: f32,
    bscale: f32,
    bitpix: Bitpix,
    /// Byte offset of the first pixel in the file.
    data_offset: u64,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Decode a FITS file into a display-ready RGB preview image.
///
/// Only the rows that contribute to the output are read from disk.
/// Output is capped at [`MAX_DISPLAY_DIM`] in each axis.
pub fn decode_preview(path: &Path) -> Result<PreviewImage, FitsError> {
    let info = read_fits_info(path)?;

    // Block size in raw sensor coordinates per output pixel.
    let (bw, bh) = block_size(&info);

    let (rgb_f32, out_w, out_h) = if info.is_rgb3d {
        read_3d_strided(&info, path, bw, bh)?
    } else {
        read_2d_strided(&info, path, bw, bh)?
    };

    Ok(PreviewImage {
        width: out_w,
        height: out_h,
        pixels: autostretch(&rgb_f32),
    })
}

// ---------------------------------------------------------------------------
// Header reading
// ---------------------------------------------------------------------------

fn read_fits_info(path: &Path) -> Result<FitsInfo, FitsError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut fits = Fits::from_reader(reader);

    let primary_hdu = match fits.next().ok_or(FitsError::MissingPrimaryHDU)?? {
        HDU::Primary(img) => img,
        _ => return Err(FitsError::MissingPrimaryHDU),
    };

    let header = primary_hdu.get_header();
    let ext = header.get_xtension();
    let naxis = ext.get_naxis();

    let width = naxis.first().copied().unwrap_or(0) as usize;
    let height = naxis.get(1).copied().unwrap_or(0) as usize;
    let is_rgb3d = naxis.len() >= 3 && naxis[2] == 3;
    let bitpix = ext.get_bitpix();

    let bayer = match header.get("BAYERPAT") {
        Some(Value::String { value, .. }) => BayerPattern::from_header(value),
        _ => None,
    };

    let bzero = match header.get("BZERO") {
        Some(Value::Float { value, .. }) => *value as f32,
        Some(Value::Integer { value, .. }) => *value as f32,
        _ => 0.0_f32,
    };
    let bscale = match header.get("BSCALE") {
        Some(Value::Float { value, .. }) => *value as f32,
        Some(Value::Integer { value, .. }) => *value as f32,
        _ => 1.0_f32,
    };

    let data_offset = primary_hdu.get_data_unit_byte_offset();

    if width == 0 || height == 0 {
        return Err(FitsError::MissingPrimaryHDU);
    }

    Ok(FitsInfo { width, height, is_rgb3d, bayer, bzero, bscale, bitpix, data_offset })
}

// ---------------------------------------------------------------------------
// Stride / block size
// ---------------------------------------------------------------------------

/// Returns `(block_w, block_h)` — the number of raw sensor pixels per output pixel.
///
/// For Bayer images the block must be an even multiple of 2 so the superpixel
/// operation always lands on the same Bayer phase (RGGB → R at top-left, etc.).
fn block_size(info: &FitsInfo) -> (usize, usize) {
    // Effective output size after the mandatory 2×2 Bayer superpixel step.
    let (eff_w, eff_h) = if info.bayer.is_some() {
        (info.width / 2, info.height / 2)
    } else {
        (info.width, info.height)
    };

    // Additional down-factor needed to reach ≤ MAX_DISPLAY_DIM.
    let k = eff_w.max(eff_h).div_ceil(MAX_DISPLAY_DIM);
    let k = k.max(1);

    if info.bayer.is_some() {
        // Bayer: block must be 2·k × 2·k (always even).
        (k * 2, k * 2)
    } else {
        (k, k)
    }
}

// ---------------------------------------------------------------------------
// 2-D strided reader (mono + Bayer)
// ---------------------------------------------------------------------------

/// Read a 2-D FITS image (mono or Bayer) with row-level striding.
///
/// Only `out_h` rows are fetched from disk; for Bayer each fetch reads 2
/// consecutive rows (the superpixel pair) in a single sequential pass.
fn read_2d_strided(
    info: &FitsInfo,
    path: &Path,
    bw: usize,
    bh: usize,
) -> Result<(Vec<[f32; 3]>, usize, usize), FitsError> {
    let bpp = info.bitpix.byte_size();
    let row_bytes = info.width * bpp;

    let out_w = info.width / bw;
    let out_h = info.height / bh;

    let mut file = File::open(path)?;
    // Reusable row buffers — only a few KB even for large sensors.
    let mut row_a = vec![0u8; row_bytes];
    let mut row_b = vec![0u8; row_bytes]; // only used for Bayer

    let mut output: Vec<[f32; 3]> = Vec::with_capacity(out_w * out_h);

    for oy in 0..out_h {
        let raw_row = oy * bh; // first raw row for this output row

        // Seek to raw_row and read it.
        file.seek(SeekFrom::Start(info.data_offset + (raw_row * row_bytes) as u64))?;
        file.read_exact(&mut row_a)?;

        if info.bayer.is_some() {
            // Row b (raw_row + 1) is immediately after row a — no extra seek.
            file.read_exact(&mut row_b)?;
        }

        for ox in 0..out_w {
            let col = ox * bw; // left-most raw column for this output column

            let pixel = if let Some(pattern) = info.bayer {
                let tl = sample(&row_a, col, bpp, info.bitpix, info.bzero, info.bscale);
                let tr = sample(&row_a, col + 1, bpp, info.bitpix, info.bzero, info.bscale);
                let bl = sample(&row_b, col, bpp, info.bitpix, info.bzero, info.bscale);
                let br = sample(&row_b, col + 1, bpp, info.bitpix, info.bzero, info.bscale);
                superpixel(tl, tr, bl, br, pattern)
            } else {
                let v = sample(&row_a, col, bpp, info.bitpix, info.bzero, info.bscale);
                [v, v, v]
            };

            output.push(pixel);
        }
    }

    Ok((output, out_w, out_h))
}

// ---------------------------------------------------------------------------
// 3-D strided reader (NAXIS3 = 3 colour planes)
// ---------------------------------------------------------------------------

/// Read a 3-plane FITS image (R / G / B stored sequentially) with striding.
fn read_3d_strided(
    info: &FitsInfo,
    path: &Path,
    bw: usize,
    bh: usize,
) -> Result<(Vec<[f32; 3]>, usize, usize), FitsError> {
    let bpp = info.bitpix.byte_size();
    let row_bytes = info.width * bpp;
    let plane_bytes = (info.width * info.height * bpp) as u64;

    let out_w = info.width / bw;
    let out_h = info.height / bh;

    let mut file = File::open(path)?;
    let mut row_buf = vec![0u8; row_bytes];
    let mut output = vec![[0.0f32; 3]; out_w * out_h];

    // `ch` is used for both the plane byte offset and the channel slot in [f32; 3].
    #[allow(clippy::needless_range_loop)]
    for ch in 0..3usize {
        let plane_offset = info.data_offset + ch as u64 * plane_bytes;

        for oy in 0..out_h {
            let raw_row = oy * bh;
            file.seek(SeekFrom::Start(plane_offset + (raw_row * row_bytes) as u64))?;
            file.read_exact(&mut row_buf)?;

            let row_start = oy * out_w;
            for ox in 0..out_w {
                let v = sample(&row_buf, ox * bw, bpp, info.bitpix, info.bzero, info.bscale);
                output[row_start + ox][ch] = v;
            }
        }
    }

    Ok((output, out_w, out_h))
}

// ---------------------------------------------------------------------------
// Pixel helpers
// ---------------------------------------------------------------------------

/// Read a single pixel from a raw row buffer and apply BZERO / BSCALE.
#[inline]
fn sample(buf: &[u8], col: usize, bpp: usize, bitpix: Bitpix, bzero: f32, bscale: f32) -> f32 {
    let s = col * bpp;
    let raw = match bitpix {
        Bitpix::U8 => buf[s] as f32,
        Bitpix::I16 => i16::from_be_bytes([buf[s], buf[s + 1]]) as f32,
        Bitpix::I32 => i32::from_be_bytes([buf[s], buf[s + 1], buf[s + 2], buf[s + 3]]) as f32,
        Bitpix::I64 => {
            i64::from_be_bytes(buf[s..s + 8].try_into().unwrap_or([0; 8])) as f32
        }
        Bitpix::F32 => f32::from_be_bytes([buf[s], buf[s + 1], buf[s + 2], buf[s + 3]]),
        Bitpix::F64 => {
            f64::from_be_bytes(buf[s..s + 8].try_into().unwrap_or([0; 8])) as f32
        }
    };
    raw * bscale + bzero
}

/// 2×2 Bayer superpixel → one RGB triple.
#[inline]
fn superpixel(tl: f32, tr: f32, bl: f32, br: f32, pattern: BayerPattern) -> [f32; 3] {
    match pattern {
        BayerPattern::Rggb => [tl, (tr + bl) * 0.5, br],
        BayerPattern::Bggr => [br, (tr + bl) * 0.5, tl],
        BayerPattern::Grbg => [tr, (tl + br) * 0.5, bl],
        BayerPattern::Gbrg => [bl, (tl + br) * 0.5, tr],
    }
}

// ---------------------------------------------------------------------------
// Autostretch
// ---------------------------------------------------------------------------

/// Per-channel percentile clip (0.1 % – 99.9 %) + linear stretch → u8.
fn autostretch(rgb: &[[f32; 3]]) -> Vec<u8> {
    if rgb.is_empty() {
        return Vec::new();
    }

    let (lo_r, hi_r) = channel_percentiles(rgb, 0);
    let (lo_g, hi_g) = channel_percentiles(rgb, 1);
    let (lo_b, hi_b) = channel_percentiles(rgb, 2);

    let mut out = vec![0u8; rgb.len() * 3];
    out.par_chunks_mut(3).zip(rgb.par_iter()).for_each(|(chunk, pixel)| {
        chunk[0] = stretch_u8(pixel[0], lo_r, hi_r);
        chunk[1] = stretch_u8(pixel[1], lo_g, hi_g);
        chunk[2] = stretch_u8(pixel[2], lo_b, hi_b);
    });
    out
}

fn channel_percentiles(rgb: &[[f32; 3]], ch: usize) -> (f32, f32) {
    let mut vals: Vec<f32> = rgb.par_iter().map(|p| p[ch]).collect();
    vals.sort_unstable_by(f32::total_cmp);
    let n = vals.len();
    let lo = vals[(n / 1000).max(1) - 1];
    let hi = vals[(n * 999 / 1000).min(n - 1)];
    (lo, hi)
}

#[inline]
fn stretch_u8(v: f32, lo: f32, hi: f32) -> u8 {
    if hi <= lo {
        return 128;
    }
    (((v - lo) / (hi - lo)).clamp(0.0, 1.0) * 255.0) as u8
}
