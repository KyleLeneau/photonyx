use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// light_curve sequencename channel [-autoring] { -at=x,y | -wcs=ra,dec } { -refat=x,y | -refwcs=ra,dec } ...
/// light_curve sequencename channel [-autoring] -ninastars=file
/// ```
///
/// Analyses several stars with aperture photometry in a sequence of images and produces a light curve for one, calibrated by the others. The first coordinates, in pixels if **-at=** is used or in degrees if **-wcs=** is used, are for the star whose light will be plotted, the others for the comparison stars.
/// Alternatively, a list of target and reference stars can be passed in the format of the NINA exoplanet plugin star list, with the **-ninastars=** option. Siril will verify that all reference stars can be used before actually using them. A data file is created in the current directory named light_curve.dat, Siril plots the result to a PNG image if available
/// The ring radii for the annulus can either be configured in the settings or set to a factor of the reference image's FWHM if **-autoring** is passed. These autoring sizes are 4.2 time and 6.3 times the FWHM for the inner and outer radii, respectively.
///
/// See also the **setphot** command to set the same way the aperture radius size.
///
/// See also SEQPSF for operations on single star
///
/// Links: :ref:`seqpsf <seqpsf>`
///
#[derive(Builder)]
pub struct LightCurve {}

impl Command for LightCurve {
    fn name() -> &'static str {
        "light_curve"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
