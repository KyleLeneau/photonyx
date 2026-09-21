//! Reconstructing full colour from a colour-filter-array frame.
//!
//! A colour sensor measures one colour per pixel behind a Bayer filter; the
//! other two have to be inferred from the neighbours. Two methods live here,
//! for two different jobs:
//!
//! - [`Method::GradientCorrected`] reconstructs at full resolution and is what
//!   a calibration pipeline should use. Malvar-He-Cutler; see [`mhc`].
//! - [`Method::Rcd`] blends two directional estimates per pixel using a
//!   ratio-corrected model. Quiet and adaptive; see [`rcd`].
//! - [`Method::Ahd`] reconstructs twice, horizontally and vertically, and
//!   picks between them per pixel on which one left the more homogeneous
//!   neighbourhood. Slower and adaptive; see [`ahd`].
//! - [`Method::SuperPixel`] collapses each 2×2 tile to one RGB pixel, halving
//!   both dimensions. It interpolates nothing, so it cannot produce colour
//!   artifacts, and it is cheap — which is what a preview wants and why it was
//!   written first.

pub mod ahd;
pub mod bilinear;
pub mod mhc;
pub mod rcd;
pub mod superpixel;

use crate::bayer::BayerPattern;
use crate::error::ImageProcError;
use crate::frame::Frame;
use crate::runtime::ImageDims;

pub use superpixel::{super_pixel_debayer, super_pixel_debayer_u16};

/// How a CFA frame is turned into colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Method {
    /// Full-resolution gradient-corrected linear interpolation. Cheap and
    /// sharp; the default before [`Method::Rcd`] existed.
    GradientCorrected,
    /// Half-resolution 2×2 averaging. For previews.
    SuperPixel,
    /// Full-resolution averaging of nearest same-colour neighbours. Softer
    /// than [`Method::GradientCorrected`] but quieter, because averaging
    /// suppresses noise where a sharpening correction amplifies it.
    Bilinear,
    /// Full-resolution adaptive homogeneity-directed reconstruction
    /// (Hirakawa-Parks). Interpolates along edges rather than across them,
    /// at several times the cost of the linear methods.
    Ahd,
    /// Full-resolution ratio-corrected reconstruction with soft directional
    /// blending. Adaptive like [`Method::Ahd`] but blends rather than
    /// selects, which is what keeps its background quiet.
    ///
    /// The default: the most accurate of these against known truth, and the
    /// only one whose background noise matches Siril's own reconstruction.
    #[default]
    Rcd,
}

/// Demosaics `frame` in place, replacing its single CFA channel with three
/// colour channels.
///
/// The Bayer pattern is read from the frame's own header, so a frame that does
/// not declare one is an error rather than a guess — guessing wrong swaps red
/// and blue, which looks plausible enough to ship.
pub fn debayer_frame(frame: &mut Frame, method: Method) -> Result<(), ImageProcError> {
    let pattern = crate::bayer::detect(frame.header());
    debayer_frame_with(frame, method, pattern)
}

/// Demosaics `frame` using an explicitly supplied pattern.
pub fn debayer_frame_with(
    frame: &mut Frame,
    method: Method,
    pattern: BayerPattern,
) -> Result<(), ImageProcError> {
    if pattern == BayerPattern::None {
        return Err(ImageProcError::Processing(
            "frame declares no BAYERPAT; cannot demosaic".to_string(),
        ));
    }
    if frame.channels() != 1 {
        return Err(ImageProcError::Processing(format!(
            "frame already has {} channels; demosaicing needs a single CFA channel",
            frame.channels()
        )));
    }

    let (width, height) = (frame.width(), frame.height());

    match method {
        Method::GradientCorrected => {
            let rgb = mhc::demosaic(frame.channel(0), width, height, pattern)?;
            frame.replace(ImageDims::new(width, height, 3), rgb)?;
        }
        Method::Rcd => {
            let rgb = rcd::demosaic(frame.channel(0), width, height, pattern)?;
            frame.replace(ImageDims::new(width, height, 3), rgb)?;
        }
        Method::Ahd => {
            let rgb = ahd::demosaic(frame.channel(0), width, height, pattern)?;
            frame.replace(ImageDims::new(width, height, 3), rgb)?;
        }
        Method::Bilinear => {
            let rgb = bilinear::demosaic(frame.channel(0), width, height, pattern)?;
            frame.replace(ImageDims::new(width, height, 3), rgb)?;
        }
        Method::SuperPixel => {
            let (rgb, w, h) =
                superpixel::super_pixel_debayer(frame.channel(0), width, height, pattern);
            frame.replace(ImageDims::new(w, h, 3), rgb)?;
        }
    }
    Ok(())
}
