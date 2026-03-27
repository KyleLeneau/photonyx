use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// makepsf clear
/// makepsf load filename
/// makepsf save [filename]
/// makepsf blind [-l0] [-si] [-multiscale] [-lambda=] [-comp=] [-ks=] [-savepsf=]
/// makepsf stars [-sym] [-ks=] [-savepsf=]
/// makepsf manual { -gaussian | -moffat | -disc | -airy } [-fwhm=] [-angle=] [-ratio=] [-beta=] [-dia=] [-fl=] [-wl=] [-pixelsize=] [-obstruct=] [-ks=] [-savepsf=]
/// ```
///
/// Generates a PSF for use with deconvolution, any of the three methods exposed by RL, SB or WIENER commands. One of the following must be given as the first argument: **clear** (clears the existing PSF), **load** (loads a PSF from a file), **save** (saves the current PSF), **blind** (blind estimate of tke PSF), **stars** (generates a PSF based on measured stars from the image) or **manual** (generates a PSF manually based on a function and parameters).
///
/// No additional arguments are required when using the **clear** argument.
///
/// To load a previously saved PSF the **load** argument requires the PSF *filename* as a second argument. This may be in any format that Siril has been compiled with support for, but it must be square and should ideally be odd.
///
/// To save a previously generated PSF the argument **save** is used. Optionally, a filename may be provided (this must have one of the extensions ".fit", ".fits", ".fts" or ".tif") but if none is provided the PSF will be named based on the name of the open file or sequence.
///
/// For **blind**, the following optional arguments may be provided: **-l0** uses the l0 descent method, **-si** uses the spectral irregularity method, **-multiscale** configures the l0 method to do a multi-scale PSF estimate, **-lambda=** provides the regularization constant.
///
/// For PSF from detected **stars** the only optional parameter is **-sym**, which configures the PSF to be symmetric.
///
/// For a **manual** PSF, one of **-gaussian**, **-moffat**, **-disc** or **-airy** can be provided to specify the PSF function, Gaussian by default. For Gaussian or Moffat PSFs the optional arguments **-fwhm=**, **-angle=** and **-ratio=** may be provided. For Moffat PSFs the optional argument **-beta=** may also be provided. If these values are omitted, they default to the same values as in the deconvolution dialog. For disc PSFs only the argument **-fwhm=** is required, which for this function is used to set the *diameter* of the PSF. For Airy PSFs the following arguments may be provided: **-dia=** (sets the telescope diameter), **-fl=** (sets the telescope focal length), **-wl=** (sets the wavelength to calculate the Airy diffraction pattern for), **-pixelsize=** (sets the sensor pixel size), **-obstruct=** (sets the central obstruction as a percentage of the overall aperture area). If these parameters are not provided, wavelength will default to 525nm and central obstruction will default to 0%. Siril will attempt to read the others from the open image, but some imaging software may not provide all of them in which case you will get bad results, and note the metadata may not be populated for SER format videos. You will learn from experience which are safe to omit for your particular imaging setup.
///
/// For any of the above PSF generation options the optional argument **-ks=** may be provided to set the PSF dimension, and the optional argument **-savepsf=\ filename** may be used to save the generated PSF: a filename must be provided and the same filename extension requirements apply as for **makepsf save filename**
///
/// Links: :ref:`psf <psf>`, :ref:`rl <rl>`, :ref:`sb <sb>`, :ref:`wiener <wiener>`
///
#[derive(Builder)]
pub struct Makepsf {}

impl Command for Makepsf {
    fn name() -> &'static str {
        "makepsf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
