use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqplatesolve sequencename [image_center_coords] [-focal=] [-pixelsize=]
/// seqplatesolve sequencename ... [-downscale] [-order=] [-radius=] [-force] [-noreg] [-disto=]
/// seqplatesolve sequencename ... [-limitmag=[+-]] [-catalog=] [-nocrop] [-nocache]
/// seqplatesolve sequencename ... [-localasnet [-blindpos] [-blindres]]
/// ```
///
/// Plate solve a sequence. A new sequence will be created with the prefix "ps\_" if the input sequence is SER, otherwise, the images headers will be updated. In case of SER, providing the metadata is mandatory and the output sequence will be in the FITS cube format, as SER cannot store WCS data.
/// If WCS or other image metadata are erroneous or missing, arguments must be passed:
/// the approximate image center coordinates can be provided in decimal degrees or degree/hour minute second values (J2000 with colon separators), with right ascension and declination values separated by a comma or a space (not mandatory for astrometry.net).
/// focal length and pixel size can be passed with **-focal=** (in mm) and **-pixelsize=** (in microns), overriding values from images and settings. See also options to solve blindly with local Astrometry.net
///
/// For faster star detection in big images, downsampling the image is possible with **-downscale**.
/// The solve can account for distortions using SIP convention with polynomials up to order 5. Default value is taken form the astrometry preferences. This can be changed with the option **-order=** giving a value between 1 and 5.
/// When using Siril solver local catalogues or with local Astrometry.net, if the initial solve is not successful, the solver will search for a solution within a cone of radius specified with **-radius=** option. If no value is passed, the search radius is taken from the astrometry preferences. Siril near search can be disabled by passing a value of 0. (cannot be disabled for Astrometry.net).
/// Images already solved will be skipped by default. This can be disabled by passing the option **-force**.
/// Using this command will update registration data unless the option **-noreg** is passed.
/// You can save the current solution as a distortion file with the option **-disto=**.
///
/// Images can be either plate solved by Siril using a star catalogue and the global registration algorithm or by astrometry.net's local solve-field command (enabled with **-localasnet**).
///
/// **Siril platesolver options:**
/// The limit magnitude of stars used for plate solving is automatically computed from the size of the field of view, but can be altered by passing a +offset or -offset value to **-limitmag=**, or simply an absolute positive value for the limit magnitude.
/// The choice of the star catalog is automatic unless the **-catalog=** option is passed: if local catalogs are installed, they are used, otherwise the choice is based on the field of view and limit magnitude. If the option is passed, it forces the use of the remote catalog given in argument, with possible values: tycho2, nomad, gaia, ppmxl, brightstars, apass.
/// If the computed field of view is larger than 5 degrees, star detection will be bounded to a cropped area around the center of the image unless **-nocrop** option is passed.
/// When using online catalogues, a single catalogue extraction will be done for the entire sequence. If there is a lot of drift or different sampling, that may not succeed for all images. This can be disabled by passing the argument **-nocache**, in which case metadata from each image will be used (except for the forced values like center coordinates, pixel size and/or focal length).
///
/// **Astrometry.net solver options:**
/// Passing options **-blindpos** and/or **-blindres** enables to solve blindly for position and for resolution respectively. You can use these when solving an image with a completely unknown location and sampling
///
#[derive(Builder)]
pub struct Seqplatesolve {}

impl Command for Seqplatesolve {
    fn name() -> &'static str {
        "seqplatesolve"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
