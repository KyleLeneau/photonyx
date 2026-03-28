use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// platesolve [-force] [image_center_coords] [-focal=] [-pixelsize=]
/// platesolve ... [-noflip] [-downscale] [-order=] [-radius=] [-disto=]
/// platesolve ... [-limitmag=[+-]] [-catalog=] [-nocrop]
/// platesolve ... [-localasnet [-blindpos] [-blindres]]
/// ```
///
/// Plate solve the loaded image.
/// If the image has already been plate solved nothing will be done, unless the **-force** argument is passed to force a new solve. If WCS or other image metadata is erroneous or missing, arguments must be passed:
/// the approximate image center coordinates can be provided in decimal degrees or degree/hour minute second values (J2000 with colon separators), with right ascension and declination values separated by a comma or a space (not mandatory for astrometry.net).
/// focal length and pixel size can be passed with **-focal=** (in mm) and **-pixelsize=** (in microns), overriding values from image and settings. See also options to solve blindly with local Astrometry.net
///
/// Unless **-noflip** is specified, if the image is detected as being upside-down, it will be flipped.
/// For faster star detection in big images, downsampling the image is possible with **-downscale**.
/// The solve can account for distortions using SIP convention with polynomials up to order 5. Default value is taken form the astrometry preferences. This can be changed with the option **-order=** giving a value between 1 and 5.
/// When using Siril solver local catalogues or with local Astrometry.net, if the initial solve is not successful, the solver will search for a solution within a cone of radius specified with **-radius=** option. If no value is passed, the search radius is taken from the astrometry preferences. Siril near search can be disabled by passing a value of 0. (cannot be disabled for Astrometry.net).
/// You can save the current solution as a distortion file with the option **-disto=**.
///
/// Images can be either plate solved by Siril using a star catalog and the global registration algorithm or by astrometry.net's local solve-field command (enabled with **-localasnet**).
///
/// **Siril platesolver options:**
/// The limit magnitude of stars used for plate solving is automatically computed from the size of the field of view, but can be altered by passing a +offset or -offset value to **-limitmag=**, or simply an absolute positive value for the limit magnitude.
/// The choice of the star catalog is automatic unless the **-catalog=** option is passed: if local catalogs are installed, they are used, otherwise the choice is based on the field of view and limit magnitude. If the option is passed, it forces the use of the catalog given in argument, with possible values: tycho2, nomad, localgaia, gaia, ppmxl, brightstars, apass.
/// If the computed field of view is larger than 5 degrees, star detection will be bounded to a cropped area around the center of the image unless **-nocrop** option is passed.
///
/// **Astrometry.net solver options:**
/// Passing options **-blindpos** and/or **-blindres** enables to solve blindly for position and for resolution respectively. You can use these when solving an image with a completely unknown location and sampling
///
#[derive(Builder)]
pub struct Platesolve {}

impl Command for Platesolve {
    fn name() -> &'static str {
        "platesolve"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
