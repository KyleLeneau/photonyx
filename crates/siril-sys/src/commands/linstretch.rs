use bon::Builder;

use crate::{
    Channels, ClipMode,
    commands::{Argument, Command},
};

/// ```text
/// linstretch -BP= [-sat] [-clipmode=] [channels] [-clipmode=]
/// ```
///
/// Stretches the image linearly to a new black point BP.
/// The argument **[channels]** may optionally be used to specify the channels to apply the stretch to: this may be R, G, B, RG, RB or GB. The default is all channels.
/// Optionally the parameter **-sat** may be used to apply the linear stretch to the image saturation channel. This argument only works if all channels are selected. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Linstretch {
    #[builder(start_fn)]
    black_point: f32,
    #[builder(default = false)]
    use_saturation: bool,
    clipmode: Option<ClipMode>,
    channels: Option<Channels>,
}

impl Command for Linstretch {
    fn name() -> &'static str {
        "linstretch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::option("BP", Some(self.black_point)),
            Argument::flag_option("sat", self.use_saturation),
            Argument::option("clipmode", self.clipmode.as_ref()),
            Argument::positional_option(self.channels.as_ref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_point_only() {
        let cmd = Linstretch::builder(0.1_f32).build();
        assert_eq!(cmd.to_args_string(), "linstretch -BP=0.1");
    }

    #[test]
    fn with_saturation_flag() {
        let cmd = Linstretch::builder(0.1_f32).use_saturation(true).build();
        assert_eq!(cmd.to_args_string(), "linstretch -BP=0.1 -sat");
    }

    #[test]
    fn with_clipmode() {
        let cmd = Linstretch::builder(0.05_f32)
            .clipmode(ClipMode::Clip)
            .build();
        assert_eq!(cmd.to_args_string(), "linstretch -BP=0.05 -clipmode=clip");
    }

    #[test]
    fn with_channels() {
        let cmd = Linstretch::builder(0.1_f32).channels(Channels::R).build();
        assert_eq!(cmd.to_args_string(), "linstretch -BP=0.1 R");
    }

    #[test]
    fn all_options() {
        let cmd = Linstretch::builder(0.0_f32)
            .use_saturation(true)
            .clipmode(ClipMode::RGBBlend)
            .channels(Channels::RG)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "linstretch -BP=0 -sat -clipmode=rgbblend RG"
        );
    }
}
