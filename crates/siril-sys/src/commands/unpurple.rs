use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// unpurple [-starmask] [-blue=value] [-thresh=value]
/// ```
///
/// Applies a cosmetic filter to reduce effects of purple fringing on stars.
///
/// If the **-starmask** parameter is given, a star mask will be used to identify areas of the image to affect. If a Dynamic PSF has already been run, this will be used for the starmask, otherwise one will be created automatically. The **-mod=** parameter should be given a value somewhere around 0.14 to reduce the amount of purple. The **-thresh=** will specify the size modifier for each star in the starmask and should be large enough to cause the stars to be entirely processed without remaining purple fringing. The value should between 0 and 1, typically around 0.5.
/// If the **-starmask** parameter is not given, the purple reduction will be applied across the entire image for any purple pixels with a luminance value higher than the given **-thresh=**. In this case, the **-thresh=** value should be reasonably low. This mode is useful for starmasks or other images without nebula or galaxy
///
/// Links: :ref:`psf <psf>`
///
#[derive(Builder)]
pub struct Unpurple {
    #[builder(default = false)]
    starmask: bool,
    blue: Option<f64>,
    threshold: Option<f64>,
}

impl Command for Unpurple {
    fn name() -> &'static str {
        "unpurple"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::flag_option("starmask", self.starmask),
            Argument::option("blue", self.blue),
            Argument::option("thresh", self.threshold)
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_no_starmask_no_optional_args() {
        let cmd = Unpurple::builder().build();
        assert_eq!(cmd.to_args_string(), "unpurple");
    }

    #[test]
    fn starmask_flag_included_when_true() {
        let cmd = Unpurple::builder().starmask(true).build();
        assert_eq!(cmd.to_args_string(), "unpurple -starmask");
    }

    #[test]
    fn blue_option_only() {
        let cmd = Unpurple::builder().blue(0.14_f64).build();
        assert_eq!(cmd.to_args_string(), "unpurple -blue=0.14");
    }

    #[test]
    fn threshold_option_only() {
        let cmd = Unpurple::builder().threshold(0.5_f64).build();
        assert_eq!(cmd.to_args_string(), "unpurple -thresh=0.5");
    }

    #[test]
    fn starmask_with_all_options() {
        let cmd = Unpurple::builder()
            .starmask(true)
            .blue(0.14_f64)
            .threshold(0.5_f64)
            .build();
        assert_eq!(cmd.to_args_string(), "unpurple -starmask -blue=0.14 -thresh=0.5");
    }

    #[test]
    fn blue_and_threshold_without_starmask() {
        let cmd = Unpurple::builder()
            .blue(0.1_f64)
            .threshold(0.3_f64)
            .build();
        assert_eq!(cmd.to_args_string(), "unpurple -blue=0.1 -thresh=0.3");
    }
}
