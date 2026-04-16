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

    let mut pixels = autostretch(&rgb_f32);
    suppress_hot_pixels(&mut pixels, out_w, out_h);

    Ok(PreviewImage { width: out_w, height: out_h, pixels })
}

pub fn decode_preview_alt(path: &Path) -> Result<PreviewImage, FitsError> {
    use astroimage::BayerPattern;
    use astroimage::ImageConverter;

    // Read raw pixels and metadata in one pass so we can compute the
    // downscale factor needed to respect MAX_DISPLAY_DIM before processing.
    let (meta, pixels) = ImageConverter::read_raw(path)
        .map_err(|e| FitsError::Processing(e.to_string()))?;

    let is_bayer = meta.bayer_pattern != BayerPattern::None;
    let max_dim = meta.width.max(meta.height);

    // Factor required to bring the longest axis within MAX_DISPLAY_DIM.
    // For Bayer images the debayer step halves dimensions internally, so the
    // pipeline applies `extra = factor / 2` as an additional reduction.
    // This means factor must be even (an odd value truncates to the same
    // effective reduction as factor − 1) and at least 2.
    let factor = {
        let f = max_dim.div_ceil(MAX_DISPLAY_DIM).max(1);
        if is_bayer {
            let even = if f % 2 == 0 { f } else { f + 1 };
            even.max(2)
        } else {
            f
        }
    };

    let image = ImageConverter::new()
        .with_downscale(2)
        .with_preview_mode()
        .process_data(meta, pixels)
        .map_err(|e| FitsError::Processing(e.to_string()))?;

    Ok(PreviewImage { width: image.width, height: image.height, pixels: image.data })
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

/// Read a 2-D FITS image (mono or Bayer) with box-filter downscaling.
///
/// All `bh` rows and all `bw` columns in each output block are read and
/// averaged, giving proper anti-aliased downscaling rather than point-sampling.
/// Peak memory stays constant because rows are accumulated one at a time.
///
/// For Bayer images `bw` and `bh` are both even, so each block contains
/// `(bw/2) × (bh/2)` superpixels; all are averaged into a single RGB output.
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
    let mut row_a = vec![0u8; row_bytes];
    let mut row_b = vec![0u8; row_bytes]; // only used for Bayer
    // Per-output-row accumulator; reset for each block of raw rows.
    let mut acc = vec![[0.0f32; 3]; out_w];

    let mut output: Vec<[f32; 3]> = Vec::with_capacity(out_w * out_h);

    for oy in 0..out_h {
        let base_row = oy * bh;

        // Reset accumulators for this output row.
        for a in acc.iter_mut() {
            *a = [0.0; 3];
        }

        if let Some(pattern) = info.bayer {
            // Bayer: process bh/2 superpixel row-pairs, each pair contributes
            // bw/2 superpixels per output column.
            let pairs = bh / 2;
            let sp_per_col = bw / 2; // superpixels averaged per output column
            let count = (pairs * sp_per_col) as f32;

            for pair in 0..pairs {
                let raw_row = base_row + pair * 2;
                file.seek(SeekFrom::Start(
                    info.data_offset + (raw_row * row_bytes) as u64,
                ))?;
                file.read_exact(&mut row_a)?;
                file.read_exact(&mut row_b)?; // row_a + 1, sequential — no extra seek

                for (ox, a) in acc.iter_mut().enumerate() {
                    let base_col = ox * bw;
                    for k in 0..sp_per_col {
                        let col = base_col + k * 2;
                        let tl = sample(&row_a, col, bpp, info.bitpix, info.bzero, info.bscale);
                        let tr =
                            sample(&row_a, col + 1, bpp, info.bitpix, info.bzero, info.bscale);
                        let bl = sample(&row_b, col, bpp, info.bitpix, info.bzero, info.bscale);
                        let br =
                            sample(&row_b, col + 1, bpp, info.bitpix, info.bzero, info.bscale);
                        let rgb = superpixel(tl, tr, bl, br, pattern);
                        a[0] += rgb[0];
                        a[1] += rgb[1];
                        a[2] += rgb[2];
                    }
                }
            }

            for a in &acc {
                output.push([a[0] / count, a[1] / count, a[2] / count]);
            }
        } else {
            // Mono: average all bw × bh raw pixels in the block.
            let count = (bw * bh) as f32;

            for dy in 0..bh {
                let raw_row = base_row + dy;
                file.seek(SeekFrom::Start(
                    info.data_offset + (raw_row * row_bytes) as u64,
                ))?;
                file.read_exact(&mut row_a)?;

                for (ox, a) in acc.iter_mut().enumerate() {
                    let base_col = ox * bw;
                    let mut sum = 0.0f32;
                    for dx in 0..bw {
                        sum +=
                            sample(&row_a, base_col + dx, bpp, info.bitpix, info.bzero, info.bscale);
                    }
                    a[0] += sum;
                }
            }

            for a in &acc {
                let v = a[0] / count;
                output.push([v, v, v]);
            }
        }
    }

    Ok((output, out_w, out_h))
}

// ---------------------------------------------------------------------------
// 3-D strided reader (NAXIS3 = 3 colour planes)
// ---------------------------------------------------------------------------

/// Read a 3-plane FITS image (R / G / B stored sequentially) with box-filter downscaling.
///
/// All `bh` rows and all `bw` columns in each block are averaged per channel,
/// giving proper anti-aliased downscaling without increasing peak memory.
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
    let count = (bw * bh) as f32;

    let mut file = File::open(path)?;
    let mut row_buf = vec![0u8; row_bytes];
    let mut output = vec![[0.0f32; 3]; out_w * out_h];

    // `ch` is used for both the plane byte offset and the channel slot in [f32; 3].
    #[allow(clippy::needless_range_loop)]
    for ch in 0..3usize {
        let plane_offset = info.data_offset + ch as u64 * plane_bytes;

        for oy in 0..out_h {
            let base_row = oy * bh;
            let row_start = oy * out_w;
            let mut acc = vec![0.0f32; out_w];

            for dy in 0..bh {
                file.seek(SeekFrom::Start(
                    plane_offset + ((base_row + dy) * row_bytes) as u64,
                ))?;
                file.read_exact(&mut row_buf)?;

                for (ox, a) in acc.iter_mut().enumerate() {
                    let base_col = ox * bw;
                    for dx in 0..bw {
                        *a +=
                            sample(&row_buf, base_col + dx, bpp, info.bitpix, info.bzero, info.bscale);
                    }
                }
            }

            for ox in 0..out_w {
                output[row_start + ox][ch] = acc[ox] / count;
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

/// Stretch parameters for a single channel.
struct StretchParams {
    /// Clipped black point (sky background level).
    black: f32,
    /// Linear scale factor: `1 / (white − black)`.
    scale: f32,
    /// MTF midpoint that maps the sky median to 25 % output brightness.
    midtone: f32,
}

/// MTF autostretch with luminance-linked scale and per-channel black points.
///
/// Algorithm:
/// 1. Compute a **per-channel black point** (`median − 2.8 × MADN`) so that each
///    channel's noise floor is removed independently.
/// 2. Build the **black-subtracted luminance** `L = (R' + G' + B') / 3` and
///    compute its statistics for a single shared scale and MTF midtone.
/// 3. Apply the shared scale and MTF to all three channels.
///
/// Linking scale and midtone to luminance preserves the relative colour ratios
/// of stars — a physically white star (equal ADU in all channels above its own
/// black level) will render white rather than with a colour cast caused by
/// independent per-channel normalisation.
fn autostretch(rgb: &[[f32; 3]]) -> Vec<u8> {
    if rgb.is_empty() {
        return Vec::new();
    }

    let params = compute_linked_stretch_params(rgb);

    let mut out = vec![0u8; rgb.len() * 3];
    out.par_chunks_mut(3).zip(rgb.par_iter()).for_each(|(chunk, pixel)| {
        for (i, p) in params.iter().enumerate() {
            let x = ((pixel[i] - p.black) * p.scale).clamp(0.0, 1.0);
            chunk[i] = (mtf(x, p.midtone) * 255.0) as u8;
        }
    });
    out
}

/// Build `[StretchParams; 3]` with independent black points but a shared scale
/// and MTF midtone derived from the raw luminance.
///
/// Statistics are computed on the raw (non-black-subtracted) luminance so that
/// `norm_median` sits at the same position in the range as the original per-channel
/// approach — avoiding the over-brightening that occurs when the median of an
/// already-black-subtracted signal is near zero.
fn compute_linked_stretch_params(rgb: &[[f32; 3]]) -> [StretchParams; 3] {
    let blacks: [f32; 3] = std::array::from_fn(|ch| channel_black_point(rgb, ch));

    // Raw (non-black-subtracted) luminance for scale and midtone statistics.
    let mut lum: Vec<f32> = rgb
        .iter()
        .map(|p| (p[0] + p[1] + p[2]) / 3.0)
        .filter(|v| v.is_finite())
        .collect();

    if lum.is_empty() {
        return std::array::from_fn(|_| StretchParams { black: 0.0, scale: 1.0, midtone: 0.5 });
    }

    lum.sort_unstable_by(f32::total_cmp);
    let n = lum.len();

    let median = lum[n / 2];
    let madn = {
        let mut devs: Vec<f32> = lum.iter().map(|&v| (v - median).abs()).collect();
        devs.sort_unstable_by(f32::total_cmp);
        devs[n / 2] * 1.4826
    };

    let black_lum = (median - 2.8 * madn).max(lum[0]);
    let white_lum = lum[(n * 999 / 1000).min(n - 1)];
    let range = (white_lum - black_lum).max(1e-6);

    // norm_median = (median − black_lum) / range  ≈  2.8 · MADN / range
    // This mirrors the per-channel calculation and places the sky background
    // at the correct (low) position in the normalised range.
    let norm_median = ((median - black_lum) / range).clamp(0.001, 0.999);
    let midtone = {
        let t = 0.25_f32;
        let m = norm_median * (t - 1.0) / (2.0 * norm_median * t - norm_median - t);
        m.clamp(0.001, 0.999)
    };

    // scale is 1 / luminance_range; applying it to (pixel[ch] − black_ch) means
    // equal-ADU channels normalise to the same value → white stars stay white.
    let scale = 1.0 / range;

    std::array::from_fn(|ch| StretchParams { black: blacks[ch], scale, midtone })
}

/// Compute the black point for one channel via `median − 2.8 × MADN`.
fn channel_black_point(rgb: &[[f32; 3]], ch: usize) -> f32 {
    let mut vals: Vec<f32> =
        rgb.iter().map(|p| p[ch]).filter(|v| v.is_finite()).collect();

    if vals.len() < 2 {
        return 0.0;
    }

    vals.sort_unstable_by(f32::total_cmp);
    let n = vals.len();
    let median = vals[n / 2];
    let madn = {
        let mut devs: Vec<f32> = vals.iter().map(|&v| (v - median).abs()).collect();
        devs.sort_unstable_by(f32::total_cmp);
        devs[n / 2] * 1.4826
    };
    (median - 2.8 * madn).max(vals[0])
}

/// Midtone Transfer Function used by PixInsight / Siril.
///
/// `MTF(x, m) = (m − 1) · x / ((2m − 1) · x − m)`
///
/// Maps `[0, 1] → [0, 1]` with a smooth rational curve.
/// - `m = 0.5` → identity (linear)
/// - `m < 0.5` → brightens midtones, compresses highlights
/// - `m > 0.5` → darkens midtones
#[inline]
fn mtf(x: f32, m: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let denom = (2.0 * m - 1.0) * x - m;
    if denom.abs() < 1e-7 {
        return x; // near-linear region; avoid division by zero
    }
    ((m - 1.0) * x / denom).clamp(0.0, 1.0)
}

// ---------------------------------------------------------------------------
// Hot pixel suppression
// ---------------------------------------------------------------------------

/// Replace isolated single-channel bright outliers with the 3×3 neighbourhood median.
///
/// A pixel is considered a hot pixel when one channel is both ≥ 200/255 and more
/// than 100 DN above the median of its own three channels — the signature of a
/// raw sensor photosite stuck at a high value rather than a real astronomical
/// object (which illuminates all channels similarly).  Stars span several pixels
/// so their cores survive the neighbourhood median intact.
fn suppress_hot_pixels(pixels: &mut [u8], width: usize, height: usize) {
    if width < 3 || height < 3 {
        return;
    }

    let orig = pixels.to_vec();

    for y in 1..height - 1 {
        for x in 1..width - 1 {
            let idx = (y * width + x) * 3;
            let r = orig[idx] as i32;
            let g = orig[idx + 1] as i32;
            let b = orig[idx + 2] as i32;

            // Median of the three channels at this pixel.
            let mut ch_vals = [r, g, b];
            ch_vals.sort_unstable();
            let ch_median = ch_vals[1];

            for ch in 0..3usize {
                let v = orig[idx + ch] as i32;
                // Hot-pixel criterion: near-saturation AND single-channel outlier.
                if v >= 200 && v - ch_median > 100 {
                    let mut nb = [0u8; 9];
                    let offsets: [(i32, i32); 9] = [
                        (-1, -1), (-1, 0), (-1, 1),
                        ( 0, -1), ( 0, 0), ( 0, 1),
                        ( 1, -1), ( 1, 0), ( 1, 1),
                    ];
                    for (i, (dy, dx)) in offsets.iter().enumerate() {
                        let ny = (y as i32 + dy) as usize;
                        let nx = (x as i32 + dx) as usize;
                        nb[i] = orig[(ny * width + nx) * 3 + ch];
                    }
                    nb.sort_unstable();
                    pixels[idx + ch] = nb[4]; // median of 9
                }
            }
        }
    }
}
