use bon::Builder;

use crate::{
    SigmaRange,
    commands::{Argument, Command},
};

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
pub struct CalibrateSingle {
    #[builder(start_fn, into)]
    image_name: String,
    #[builder(into)]
    bias: Option<String>,
    #[builder(into)]
    dark: Option<String>,
    #[builder(into)]
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
    #[builder(into)]
    prefix: Option<String>,
    #[builder(default = false)]
    cosmetic_correction_from_dark: bool,
    cosmetic_correction_from_dark_range: Option<SigmaRange>,
    #[builder(into)]
    cosmetic_correction_from_bad_pixel_map: Option<String>,
}

impl Command for CalibrateSingle {
    fn name() -> &'static str {
        "calibrate_single"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![
            Argument::positional(&self.image_name),
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
                args.push(Argument::positional(range.low.to_string()));
                args.push(Argument::positional(range.high.to_string()));
            }
        }

        if let Some(bpm) = &self.cosmetic_correction_from_bad_pixel_map {
            args.push(Argument::option("cc", Some("bpm")));
            args.push(Argument::positional(bpm));
        }

        args.extend([
            Argument::flag_option("cfa", self.cfa),
            Argument::flag_option("debayer", self.debayer),
            Argument::flag_option("fix_xtrans", self.fix_xtrans),
            Argument::flag_option("equalize_cfa", self.equalize_cfa),
            Argument::flag_option("opt", self.dark_optimization),
            Argument::option("prefix", self.prefix.as_deref()),
        ]);

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_image_only() {
        let cmd = CalibrateSingle::builder("light.fit").build();
        assert_eq!(cmd.to_args_string(), "calibrate_single light.fit");
    }

    #[test]
    fn with_bias_dark_flat() {
        let cmd = CalibrateSingle::builder("light.fit")
            .bias("bias.fit")
            .dark("dark.fit")
            .flat("flat.fit")
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "calibrate_single light.fit -bias=bias.fit -dark=dark.fit -flat=flat.fit"
        );
    }

    #[test]
    fn cosmetic_correction_from_dark_no_range() {
        let cmd = CalibrateSingle::builder("light.fit")
            .dark("dark.fit")
            .cosmetic_correction_from_dark(true)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "calibrate_single light.fit -dark=dark.fit -cc=dark"
        );
    }

    #[test]
    fn cosmetic_correction_from_dark_with_range() {
        let cmd = CalibrateSingle::builder("light.fit")
            .dark("dark.fit")
            .cosmetic_correction_from_dark(true)
            .cosmetic_correction_from_dark_range(SigmaRange {
                low: 2.0,
                high: 3.0,
            })
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "calibrate_single light.fit -dark=dark.fit -cc=dark 2 3"
        );
    }

    #[test]
    fn cosmetic_correction_from_bad_pixel_map() {
        let cmd = CalibrateSingle::builder("light.fit")
            .cosmetic_correction_from_bad_pixel_map("bpm.fit")
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "calibrate_single light.fit -cc=bpm bpm.fit"
        );
    }

    #[test]
    fn cc_bpm_takes_priority_over_dark() {
        // When both are set, bpm branch wins (dark cc is skipped)
        let cmd = CalibrateSingle::builder("light.fit")
            .dark("dark.fit")
            .cosmetic_correction_from_dark(true)
            .cosmetic_correction_from_bad_pixel_map("bpm.fit")
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-cc=bpm bpm.fit"));
        assert!(!s.contains("-cc=dark"));
    }

    #[test]
    fn color_flags() {
        let cmd = CalibrateSingle::builder("light.fit")
            .cfa(true)
            .debayer(true)
            .equalize_cfa(true)
            .fix_xtrans(true)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-cfa"));
        assert!(s.contains("-debayer"));
        assert!(s.contains("-equalize_cfa"));
        assert!(s.contains("-fix_xtrans"));
    }

    #[test]
    fn dark_optimization_flag() {
        let cmd = CalibrateSingle::builder("light.fit")
            .dark_optimization(true)
            .build();
        assert!(cmd.to_args_string().contains("-opt"));
    }

    #[test]
    fn custom_prefix() {
        let cmd = CalibrateSingle::builder("light.fit").prefix("cal_").build();
        assert_eq!(
            cmd.to_args_string(),
            "calibrate_single light.fit -prefix=cal_"
        );
    }

    #[test]
    fn image_name_with_spaces_is_quoted() {
        let cmd = CalibrateSingle::builder("my light.fit").build();
        assert_eq!(cmd.to_args_string(), "calibrate_single 'my light.fit'");
    }
}
