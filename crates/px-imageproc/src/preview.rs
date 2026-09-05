//! Decode a FITS file into a display-ready RGB preview (ADR 006, Phase 9,
//! P9-T4/P9-T7).
//!
//! Ports `astroimage`'s preview path onto native `px-fits` reads: Bayer
//! debayer ([`crate::debayer`]), STF autostretch ([`crate::stretch`]), and
//! integer downscale/binning ([`crate::downscale`], [`crate::binning`]) to
//! respect [`MAX_DISPLAY_DIM`]. `BSCALE`/`BZERO` application and BITPIX
//! decoding are `px-fits`'s job (`ImageHdu::read_full::<f32>()`); this
//! module only interprets the resulting physical pixel values.

use std::path::Path;

use px_fits::header::BitPix;
use px_fits::reader::FitsReader;

use crate::bayer::{self, BayerPattern};
use crate::error::ImageProcError;
use crate::{binning, color, debayer, downscale, stretch};

/// Maximum output dimension in either axis.
pub const MAX_DISPLAY_DIM: usize = 2048;

/// The assumed native full-scale value for STF normalization, matching
/// `astroimage`'s convention: stretch parameters are computed as if every
/// image were 16-bit, regardless of the FITS file's actual `BITPIX`.
const STRETCH_MAX_INPUT: f32 = 65536.0;

/// A decoded, display-ready image with interleaved RGB bytes.
pub struct PreviewImage {
    pub width: usize,
    pub height: usize,
    /// Interleaved RGB bytes, length = `width * height * 3`.
    pub pixels: Vec<u8>,
}

/// Decode a FITS file into a display-ready RGB preview image.
///
/// Output is capped at [`MAX_DISPLAY_DIM`] in each axis. Bayer debayering,
/// BITPIX conversion, and autostretch are applied by this crate.
pub fn decode_preview(path: &Path) -> Result<PreviewImage, ImageProcError> {
    let reader = FitsReader::open(path)?;
    let image = reader.primary_image()?;
    let header = image.header();

    let shape = image.shape();
    if shape.len() < 2 {
        return Err(ImageProcError::Processing(format!(
            "image has {} axes, need at least 2",
            shape.len()
        )));
    }
    let width = shape[0];
    let height = shape[1];
    let channels = match shape.get(2) {
        Some(&n) if n > 0 => n,
        _ => 1,
    };

    let bayer_pattern = bayer::detect(header);
    let flip_vertical = header.get_string("ROWORDER").as_deref() == Some("TOP-DOWN");
    let is_bayer = channels == 1 && bayer_pattern != BayerPattern::None;
    // Debayering straight from the on-disk integer type (skipping a
    // whole-frame f32 conversion first) roughly halves preview latency for
    // the common case: 16-bit-unsigned raw from a one-shot-color camera.
    let is_integer_bitpix = !matches!(image.bitpix(), BitPix::F32 | BitPix::F64);

    // Factor required to bring the longest axis within MAX_DISPLAY_DIM.
    let max_dim = width.max(height);
    let f = ((max_dim as f64 / MAX_DISPLAY_DIM as f64).ceil() as usize).max(1);
    let factor = if is_bayer {
        // Debayer halves dims internally (extra = factor / 2), so the
        // overall factor must be even and at least 2.
        let even = if f.is_multiple_of(2) { f } else { f + 1 };
        even.max(2)
    } else {
        f
    };

    let (float_data, out_w, out_h, is_color) = if is_bayer && is_integer_bitpix {
        let data = image.read_full::<u16>()?;
        let (mut rgb, mut ow, mut oh) =
            debayer::super_pixel_debayer_u16(&data, width, height, bayer_pattern);
        let extra = factor / 2;
        if extra > 1 {
            let (d, nw, nh) = downscale::downscale_planar(&rgb, ow, oh, 3, extra);
            rgb = d;
            ow = nw;
            oh = nh;
        }
        (rgb, ow, oh, true)
    } else if is_bayer {
        let data = image.read_full::<f32>()?;
        let (mut rgb, mut ow, mut oh) =
            debayer::super_pixel_debayer(&data, width, height, bayer_pattern);
        let extra = factor / 2;
        if extra > 1 {
            let (d, nw, nh) = downscale::downscale_planar(&rgb, ow, oh, 3, extra);
            rgb = d;
            ow = nw;
            oh = nh;
        }
        (rgb, ow, oh, true)
    } else if channels == 3 {
        let mut rgb = image.read_full::<f32>()?;
        let (mut ow, mut oh) = (width, height);
        if factor > 1 {
            let (d, nw, nh) = downscale::downscale_planar(&rgb, width, height, 3, factor);
            rgb = d;
            ow = nw;
            oh = nh;
        }
        (rgb, ow, oh, true)
    } else {
        let mut mono = image.read_full::<f32>()?;
        let (mut ow, mut oh) = (width, height);
        if factor > 1 {
            let (d, nw, nh) = downscale::downscale_planar(&mono, width, height, 1, factor);
            mono = d;
            ow = nw;
            oh = nh;
        }
        // decode_preview always previews, so mono always gets an extra 2x2
        // bin on top of the MAX_DISPLAY_DIM downscale (matches
        // astroimage's `preview_mode` mono path).
        let (binned, nw, nh) = binning::bin_2x2(&mono, ow, oh);
        (binned, nw, nh, false)
    };

    let channel_size = out_w * out_h;
    let mut out_data = vec![0u8; channel_size * 3];

    if is_color {
        let coeffs = stretch::compute_stretch_coeffs_per_channel(
            &float_data,
            3,
            channel_size,
            STRETCH_MAX_INPUT,
        );
        for (c, coeff) in coeffs.iter().enumerate() {
            let ch = &float_data[c * channel_size..(c + 1) * channel_size];
            stretch::apply_stretch(ch, &mut out_data, c, 3, coeff);
        }
    } else {
        let coeffs = stretch::StretchCoeffs::compute(&float_data, STRETCH_MAX_INPUT);
        let mut gray = vec![0u8; channel_size];
        stretch::apply_stretch(&float_data, &mut gray, 0, 1, &coeffs);
        out_data = color::replicate_gray_to_rgb(&gray);
    }

    if flip_vertical {
        color::vertical_flip(&mut out_data, out_w, out_h, 3);
    }

    Ok(PreviewImage {
        width: out_w,
        height: out_h,
        pixels: out_data,
    })
}
