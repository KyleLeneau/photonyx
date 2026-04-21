use bon::Builder;

use crate::{
    ClipMode,
    commands::{Argument, Command},
};

/// ```text
/// autoghs [-linked] shadowsclip stretchamount [-b=] [-hp=] [-lp=] [-clipmode=]
/// ```
///
/// Application of the generalized hyperbolic stretch with a symmetry point SP defined as k.sigma from the median of each channel (the provided **shadowsclip** value is the k here and can be negative). By default, SP and the stretch are computed per channel; SP can be computed as a mean of image channels by passing **-linked**. The stretch amount **D** is provided in the second mandatory argument.
/// Implicit values of 13 for **B**, making it very focused on the SP brightness range, 0.7 for **HP**, 0 for **LP** are used but can be changed with the options of the same names. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Autoghs {
    #[builder(start_fn)]
    shadows_clipping: f32,
    #[builder(start_fn)]
    stretch_amount: f32,
    #[builder(default = false)]
    linked: bool,
    black_point: Option<f32>,
    high_point: Option<f32>,
    low_point: Option<f32>,
    clipmode: Option<ClipMode>,
}

impl Command for Autoghs {
    fn name() -> &'static str {
        "autoghs"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::flag_option("linked", self.linked),
            Argument::positional(self.shadows_clipping.to_string()),
            Argument::positional(self.stretch_amount.to_string()),
            Argument::option("b", self.black_point),
            Argument::option("hp", self.high_point),
            Argument::option("lp", self.low_point),
            Argument::option("clipmode", self.clipmode.as_ref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_args_only() {
        let cmd = Autoghs::builder(-2.8f32, 5.0f32).build();
        assert_eq!(cmd.to_args_string(), "autoghs -2.8 5");
    }

    #[test]
    fn with_linked() {
        let cmd = Autoghs::builder(-2.8f32, 5.0f32).linked(true).build();
        assert_eq!(cmd.to_args_string(), "autoghs -linked -2.8 5");
    }

    #[test]
    fn with_black_point() {
        let cmd = Autoghs::builder(-2.8f32, 5.0f32)
            .black_point(10.0f32)
            .build();
        assert_eq!(cmd.to_args_string(), "autoghs -2.8 5 -b=10");
    }

    #[test]
    fn with_high_and_low_point() {
        let cmd = Autoghs::builder(-2.8f32, 5.0f32)
            .high_point(0.7f32)
            .low_point(0.1f32)
            .build();
        assert_eq!(cmd.to_args_string(), "autoghs -2.8 5 -hp=0.7 -lp=0.1");
    }

    #[test]
    fn with_clipmode() {
        let cmd = Autoghs::builder(-2.8f32, 5.0f32)
            .clipmode(ClipMode::GlobalRescale)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "autoghs -2.8 5 -clipmode=globalrescale"
        );
    }

    #[test]
    fn all_options() {
        let cmd = Autoghs::builder(-2.8f32, 5.0f32)
            .linked(true)
            .black_point(13.0f32)
            .high_point(0.7f32)
            .low_point(0.0f32)
            .clipmode(ClipMode::RGBBlend)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "autoghs -linked -2.8 5 -b=13 -hp=0.7 -lp=0 -clipmode=rgbblend"
        );
    }
}
