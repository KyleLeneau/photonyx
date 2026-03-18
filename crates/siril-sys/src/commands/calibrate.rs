use bon::Builder;

use crate::{
    SigmaRange,
    commands::{Argument, Command},
};

/// .. code-block:: text
///
///     calibrate sequencename [-bias=filename] [-dark=filename] [-flat=filename] [-cc=dark [siglo sighi] || -cc=bpm bpmfile] [-cfa] [-debayer] [-fix_xtrans] [-equalize_cfa] [-opt[=exp]] [-all] [-prefix=] [-fitseq]
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
#[derive(Builder)]
pub struct Calibrate {
    #[builder(start_fn)]
    base_name: String,
    bias: Option<String>,
    dark: Option<String>,
    flat: Option<String>,
    #[builder(default = false)]
    cfa: bool,
    #[builder(default = false)]
    debayer: bool,
    #[builder(default = false)]
    fix_xtrans: bool,
    #[builder(default = false)]
    equalize_cfa: bool,
    #[builder(default = false)]
    dark_optimization: bool,
    #[builder(default = false)]
    all_frames: bool,
    prefix: Option<String>,
    #[builder(default = false)]
    create_fitsseq: bool,
    #[builder(default = false)]
    cosmetic_correction_from_dark: bool,
    cosmetic_correction_from_dark_range: Option<SigmaRange>,
    cosmetic_correction_from_bad_pixel_map: Option<String>,
}

impl Command for Calibrate {
    fn name() -> &'static str {
        "calibrate"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![
            Argument::positional(&self.base_name),
            Argument::option("bias", self.bias.as_deref()),
            Argument::option("dark", self.dark.as_deref()),
            Argument::option("flat", self.flat.as_deref()),
        ];

        if self.dark.is_some()
            && self.cosmetic_correction_from_dark
            && self.cosmetic_correction_from_bad_pixel_map.is_none()
        {
            args.push(Argument::option("cc", Some("dark")));
            if let Some(range) = &self.cosmetic_correction_from_dark_range {
                args.push(Argument::positional(range.to_string()));
            }
        }

        if let Some(bpm) = &self.cosmetic_correction_from_bad_pixel_map {
            args.push(Argument::option("cc", Some("bpm")));
            args.push(Argument::positional(bpm));
        }

        args.extend([
            Argument::flag("cfa", self.cfa),
            Argument::flag("debayer", self.debayer),
            Argument::flag("fix_xtrans", self.fix_xtrans),
            Argument::flag("equalize_cfa", self.equalize_cfa),
            Argument::flag("opt", self.dark_optimization),
            Argument::flag("all", self.all_frames),
            Argument::option("prefix", self.prefix.as_deref()),
            Argument::flag("fitseq", self.create_fitsseq),
        ]);

        args
    }
}
