use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// calibrate sequencename [-bias=filename] [-dark=filename] [-flat=filename] [-cc=dark [siglo sighi] || -cc=bpm bpmfile] [-cfa] [-debayer] [-fix_xtrans] [-equalize_cfa] [-opt[=exp]] [-all] [-prefix=] [-fitseq]
/// ```
///
/// Calibrates the sequence **sequencename** using bias, dark and flat given in argument.
///
/// For bias, a uniform level can be specified instead of an image, by entering a quoted expression starting with an = sign, such as -bias="=256" or -bias="=64*$OFFSET".
///
/// By default, cosmetic correction is not activated. If you wish to apply some, you will need to specify it with **-cc=** option.
/// You can use **-cc=dark** to detect hot and cold pixels from the masterdark (a masterdark must be given with the **-dark=** option), optionally followed by **siglo** and **sighi** for cold and hot pixels respectively. A value of 0 deactivates the correction. If sigmas are not provided, only hot pixels detection with a sigma of 3 will be applied.
/// Alternatively, you can use **-cc=bpm** followed by the path to your Bad Pixel Map to specify which pixels must be corrected. An example file can be obtained with a *find_hot* command on a masterdark.
///
/// Three options apply to color images (in CFA format): **-cfa** for cosmetic correction purposes, **-debayer** to demosaic images before saving them, and **-equalize_cfa** to equalize the mean intensity of RGB layers of the master flat, to avoid tinting the calibrated image.
/// The **-fix_xtrans** option is dedicated to X-Trans images by applying a correction on darks and biases to remove a rectangle pattern caused by autofocus.
/// It's also possible to optimize dark subtraction with **-opt**, which requires the supply of bias and dark masters, and automatically calculates the coefficient to be applied to dark, or calculates the coefficient thanks to the exposure keyword with **-opt=exp**.
/// By default, frames marked as excluded will not be processed. The argument **-all** can be used to force processing of all frames even if marked as excluded.
/// The output sequence name starts with the prefix "pp\_" unless otherwise specified with option **-prefix=**.
/// If **-fitseq** is provided, the output sequence will be a FITS sequence (single file)
///
#[derive(Builder)]
pub struct Calibrate {}

impl Command for Calibrate {
    fn name() -> &'static str {
        "calibrate"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
