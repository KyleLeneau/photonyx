use bon::Builder;

use crate::{
    ClipMode,
    commands::{Argument, Command},
};

/// ```text
/// asinh [-human] stretch { [offset] [-clipmode=] }
/// ```
///
/// Stretches the image to show faint objects using an hyperbolic arcsin transformation. The mandatory argument **stretch**, typically between 1 and 1000, will give the strength of the stretch. The black point can be offset by providing an optional **offset** argument in the normalized pixel value of [0, 1]. Finally the option **-human** enables using human eye luminous efficiency weights to compute the luminance used to compute the stretch value for each pixel, instead of the simple mean of the channels pixel values. This stretch method preserves lightness from the L\*a\*b\* color space. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Asinh {
    #[builder(start_fn, into)]
    stretch: u16,
    #[builder(default = false)]
    human_weighting: bool,
    offset: Option<u8>,
    clipmode: Option<ClipMode>,
}

impl Command for Asinh {
    fn name() -> &'static str {
        "asinh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::flag_option("human", self.human_weighting),
            Argument::positional(self.stretch.to_string()),
            Argument::positional_option(self.offset),
            Argument::option("clipmode", self.clipmode.as_ref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stretch_only() {
        let cmd = Asinh::builder(100u16).build();
        assert_eq!(cmd.to_args_string(), "asinh 100");
    }

    #[test]
    fn with_human_weighting() {
        let cmd = Asinh::builder(500u16).human_weighting(true).build();
        assert_eq!(cmd.to_args_string(), "asinh -human 500");
    }

    #[test]
    fn with_offset() {
        let cmd = Asinh::builder(200u16).offset(10u8).build();
        assert_eq!(cmd.to_args_string(), "asinh 200 10");
    }

    #[test]
    fn with_clipmode() {
        let cmd = Asinh::builder(300u16).clipmode(ClipMode::Clip).build();
        assert_eq!(cmd.to_args_string(), "asinh 300 -clipmode=clip");
    }

    #[test]
    fn all_options() {
        let cmd = Asinh::builder(1000u16)
            .human_weighting(true)
            .offset(5u8)
            .clipmode(ClipMode::RGBBlend)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "asinh -human 1000 5 -clipmode=rgbblend"
        );
    }
}
