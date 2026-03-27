use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// setfindstar [reset] [-radius=] [-sigma=] [-roundness=] [-focal=] [-pixelsize=] [-convergence=] [ [-gaussian] | [-moffat] ] [-minbeta=] [-relax=on|off] [-minA=] [-maxA=] [-maxR=]
/// ```
///
/// Defines stars detection parameters for FINDSTAR and REGISTER commands.
///
/// Passing no parameter lists the current values.
/// Passing **reset** resets all values to defaults. You can then still pass values after this keyword.
///
/// Configurable values:
///
/// **-radius=** defines the radius of the initial search box and must be between 3 and 50.
/// **-sigma=** defines the threshold above noise and must be greater than or equal to 0.05.
/// **-roundness=** defines minimum star roundness and must between 0 and 0.95. **-maxR** allows an upper bound to roundness to be set, to visualize only the areas where stars are significantly elongated, do not change for registration.
/// **-minA** and **-maxA** define limits for the minimum and maximum amplitude of stars to keep, normalized between 0 and 1.
/// **-focal=** defines the focal length of the telescope.
/// **-pixelsize=** defines the pixel size of the sensor.
/// **-gaussian** and **-moffat** configure the solver model to be used (Gaussian is the default).
/// If Moffat is selected, **-minbeta=** defines the minimum value of beta for which candidate stars will be accepted and must be greater than or equal to 0.0 and less than 10.0.
/// **-convergence=** defines the number of iterations performed to fit PSF and should be set between 1 and 3 (more tolerant).
/// **-relax=** relaxes the checks that are done on star candidates to assess if they are stars or not, to allow objects not shaped like stars to still be accepted (off by default)
///
/// Links: :ref:`findstar <findstar>`, :ref:`register <register>`, :ref:`psf <psf>`
///
/// The threshold for star detection is computed as the median of the image (which
/// represents in general the background level) plus k times sigma, sigma being the
/// standard deviation of the image (which is a good indication of the noise
/// amplitude). If you have many stars in your images and a good signal/noise
/// ratio, it may be a good idea to increase this value to speed-up the detection
/// and false positives.
///
/// It is recommended to test the values used for a sequence with Siril's GUI,
/// available in the dynamic PSF toolbox from the analysis menu. It may improve
/// registration quality to increase the parameters, but it is also important to be
/// able to detect several tens of stars in each image.
///
#[derive(Builder)]
pub struct Setfindstar {}

impl Command for Setfindstar {
    fn name() -> &'static str {
        "setfindstar"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
