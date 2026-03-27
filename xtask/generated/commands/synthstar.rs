use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// synthstar
/// ```
///
/// Fixes imperfect stars from the loaded image. No matter how much coma, tracking drift or other distortion your stars have, if Siril's star finder routine can detect it, synthstar will fix it. To use intensive care, you may wish to manually detect all the stars you wish to fix. This can be done using the findstar console command or the Dynamic PSF dialog. If you have not run star detection, it will be run automatically with default settings.
///
/// For best results synthstar should be run before stretching.
///
/// The output of synthstar is a fully corrected synthetic star mask comprising perfectly round star PSFs (Moffat or Gaussian profiles depending on star saturation) computed to match the intensity, FWHM, hue and saturation measured for each star detected in the input image. This can then be recombined with the starless image to produce an image with perfect stars.
///
/// No parameters are required for this command
///
/// Links: :ref:`psf <psf>`
///
#[derive(Builder)]
pub struct Synthstar {}

impl Command for Synthstar {
    fn name() -> &'static str {
        "synthstar"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
