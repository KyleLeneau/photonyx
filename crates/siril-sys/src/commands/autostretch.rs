use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// autostretch [-linked] [shadowsclip [targetbg]]
/// ```
///
/// Auto-stretches the currently loaded image, with different parameters for each channel (unlinked) unless **-linked** is passed. Arguments are optional, **shadowclip** is the shadows clipping point, measured in sigma units from the main histogram peak (default is -2.8), **targetbg** is the target background value, giving a final brightness to the image, range [0, 1], default is 0.25. The default values are those used in the Auto-stretch rendering from the GUI.
///
/// Do not use the unlinked version after color calibration, it will alter the white balance
///
#[derive(Builder)]
pub struct Autostretch {
    #[builder(default = false)]
    linked: bool,
    shadows_clipping: Option<f32>,
    target_background: Option<f32>,
}

impl Command for Autostretch {
    fn name() -> &'static str {
        "autostretch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::flag_option("linked", self.linked),
            Argument::positional_option(self.shadows_clipping),
            Argument::positional_option(self.target_background),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_unlinked_no_optional_args() {
        let cmd = Autostretch::builder().build();
        assert_eq!(cmd.to_args_string(), "autostretch");
    }

    #[test]
    fn linked_flag_included_when_true() {
        let cmd = Autostretch::builder().linked(true).build();
        assert_eq!(cmd.to_args_string(), "autostretch -linked");
    }

    #[test]
    fn shadows_clipping_only() {
        let cmd = Autostretch::builder().shadows_clipping(-2.8_f32).build();
        assert_eq!(cmd.to_args_string(), "autostretch -2.8");
    }

    #[test]
    fn shadows_clipping_and_target_background() {
        let cmd = Autostretch::builder()
            .shadows_clipping(-2.8_f32)
            .target_background(0.25_f32)
            .build();
        assert_eq!(cmd.to_args_string(), "autostretch -2.8 0.25");
    }

    #[test]
    fn linked_with_all_optional_args() {
        let cmd = Autostretch::builder()
            .linked(true)
            .shadows_clipping(-3.0_f32)
            .target_background(0.2_f32)
            .build();
        assert_eq!(cmd.to_args_string(), "autostretch -linked -3 0.2");
    }
}
