//! FITS display utilities: decode a FITS file into a display-ready RGB preview.
//!
//! Decoding and autostretching are delegated to the `astroimage` crate (rustafits),
//! which handles Bayer debayering, BITPIX conversion, and STF-based autostretch.
//! Output is capped at [`MAX_DISPLAY_DIM`] in each axis.

use std::path::Path;

use crate::FitsError;

/// Maximum output dimension in either axis.
pub const MAX_DISPLAY_DIM: usize = 2048;

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
/// BITPIX conversion, and autostretch are handled by the `astroimage` crate.
pub fn decode_preview(path: &Path) -> Result<PreviewImage, FitsError> {
    use astroimage::BayerPattern;
    use astroimage::ImageConverter;

    // Read raw pixels and metadata in one pass so we can compute the
    // downscale factor needed to respect MAX_DISPLAY_DIM before processing.
    let (meta, pixels) =
        ImageConverter::read_raw(path).map_err(|e| FitsError::Processing(e.to_string()))?;

    let is_bayer = meta.bayer_pattern != BayerPattern::None;
    let max_dim = meta.width.max(meta.height);

    // Factor required to bring the longest axis within MAX_DISPLAY_DIM.
    // For Bayer images the debayer step halves dimensions internally via
    // `extra = factor / 2`, so factor must be even and at least 2.
    let factor = {
        // Integer ceiling division: smallest whole number that brings the
        // longest axis to ≤ MAX_DISPLAY_DIM.
        let f = ((max_dim as f64 / MAX_DISPLAY_DIM as f64).ceil() as usize).max(1);
        if is_bayer {
            // Bayer debayer halves dims internally (extra = factor / 2),
            // so factor must be even and at least 2.
            let even = if f.is_multiple_of(2) { f } else { f + 1 };
            even.max(2)
        } else {
            f
        }
    };

    // use astroimage::ImageAnalyzer;

    // let result = ImageAnalyzer::new()
    //     .with_max_stars(500)
    //     .with_optics(403.0, 4.78)  // focal length mm, pixel size µm → arcsec output
    //     .analyze_raw(&meta, &pixels)
    //     .map_err(|e| FitsError::Processing(e.to_string()))?;

    // println!("Stars: {}  FWHM: {:.2} px ({:.1}\")  Ecc: {:.3}  Seeing: {:.1}\"",
    //     result.stars_detected, result.median_fwhm,
    //     result.median_fwhm_arcsec.unwrap_or(0.0),
    //     result.median_eccentricity,
    //     result.median_fwhm_arcsec.unwrap_or(0.0));

    // // Per-stage timing breakdown
    // let t = &result.stage_timing;
    // println!("Timing: bg={:.0}ms det={:.0}ms cal={:.0}ms meas={:.0}ms total={:.0}ms",
    //     t.background_ms, t.detection_pass1_ms, t.calibration_ms,
    //     t.measurement_ms, t.total_ms);

    let image = ImageConverter::new()
        .with_downscale(factor)
        .with_preview_mode()
        .process_data(meta, pixels)
        .map_err(|e| FitsError::Processing(e.to_string()))?;

    Ok(PreviewImage {
        width: image.width,
        height: image.height,
        pixels: image.data,
    })
}
