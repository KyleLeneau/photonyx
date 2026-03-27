use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// calibrate_single imagename [-bias=filename] [-dark=filename] [-flat=filename] [-cc=dark [siglo sighi] || -cc=bpm bpmfile] [-cfa] [-debayer] [-fix_xtrans] [-equalize_cfa] [-opt[=exp]] [-prefix=]
/// ```
///
/// Calibrates the image **imagename** using bias, dark and flat given in argument.
///
/// For bias, a uniform level can be specified instead of an image, by entering a quoted expression starting with an = sign, such as -bias="=256" or -bias="=64*".
///
/// By default, cosmetic correction is not activated. If you wish to apply some, you will need to specify it with **-cc=** option.
/// You can use **-cc=dark** to detect hot and cold pixels from the masterdark (a masterdark must be given with the **-dark=** option), optionally followed by **siglo** and **sighi** for cold and hot pixels respectively. A value of 0 deactivates the correction. If sigmas are not provided, only hot pixels detection with a sigma of 3 will be applied.
/// Alternatively, you can use **-cc=bpm** followed by the path to your Bad Pixel Map to specify which pixels must be corrected. An example file can be obtained with a *find_hot* command on a masterdark.
///
/// Three options apply to color images (in CFA format): **-cfa** for cosmetic correction purposes, **-debayer** to demosaic images before saving them, and **-equalize_cfa** to equalize the mean intensity of RGB layers of the master flat, to avoid tinting the calibrated image.
/// The **-fix_xtrans** option is dedicated to X-Trans images by applying a correction on darks and biases to remove a rectangle pattern caused by autofocus.
/// It's also possible to optimize dark subtraction with **-opt**, which requires the supply of bias and dark masters, and automatically calculates the coefficient to be applied to dark, or calculates the coefficient thanks to the exposure keyword with **-opt=exp**
/// The output filename starts with the prefix "pp\_" unless otherwise specified with option **-prefix=**
///
#[derive(Builder)]
pub struct CalibrateSingle {}

impl Command for CalibrateSingle {
    fn name() -> &'static str {
        "calibrate_single"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
