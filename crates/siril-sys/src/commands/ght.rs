use bon::Builder;

use crate::{
    Channels, ClipMode, ClipWeight, IntoArgument,
    commands::{Argument, Command},
};

/// ```text
/// ght -D= [-B=] [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels]
/// ```
///
/// Generalised hyperbolic stretch based on the work of the ghsastro.co.uk team.
///
/// The argument **-D=** defines the strength of the stretch, between 0 and 10. This is the only mandatory argument. The following optional arguments further tailor the stretch:
/// **B** defines the intensity of the stretch near the focal point, between -5 and 15;
/// **LP** defines a shadow preserving range between 0 and SP where the stretch will be linear, preserving shadow detail;
/// **SP** defines the symmetry point of the stretch, between 0 and 1, which is the point at which the stretch will be most intense;
/// **HP** defines a region between HP and 1 where the stretch is linear, preserving highlight details and preventing star bloat.
/// If omitted B, LP and SP default to 0.0 ad HP defaults to 1.0.
/// An optional argument (either **-human**, **-even** or **-independent**) can be passed to select either human-weighted or even-weighted luminance or independent colour channels for colour stretches. The argument is ignored for mono images. Alternatively, the argument **-sat** specifies that the stretch is performed on image saturation - the image must be color and all channels must be selected for this to work.
/// Optionally the parameter **[channels]** may be used to specify the channels to apply the stretch to: this may be R, G, B, RG, RB or GB. The default is all channels. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Ght {
    #[builder(start_fn)]
    strength: u8,
    focal_point: Option<f32>,
    low_point: Option<f32>,
    symetry_point: Option<f32>,
    high_point: Option<f32>,
    clipmode: Option<ClipMode>,
    weight: Option<ClipWeight>,
    channels: Option<Channels>,
}

impl Command for Ght {
    fn name() -> &'static str {
        "ght"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::option("D", Some(self.strength)),
            Argument::option("B", self.focal_point),
            Argument::option("LP", self.low_point),
            Argument::option("SP", self.symetry_point),
            Argument::option("HP", self.high_point),
            Argument::option("clipmode", self.clipmode.as_ref()),
            self.weight
                .as_ref()
                .map(ClipWeight::to_argument)
                .unwrap_or(Argument::Positional(None)),
            Argument::positional_option(self.channels.as_ref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_only() {
        let cmd = Ght::builder(5u8).build();
        assert_eq!(cmd.to_args_string(), "ght -D=5");
    }

    #[test]
    fn with_focal_point() {
        let cmd = Ght::builder(5u8).focal_point(0.5f32).build();
        assert_eq!(cmd.to_args_string(), "ght -D=5 -B=0.5");
    }

    #[test]
    fn with_all_points() {
        let cmd = Ght::builder(5u8)
            .focal_point(0.5f32)
            .low_point(0.0f32)
            .symetry_point(0.3f32)
            .high_point(1.0f32)
            .build();
        assert_eq!(cmd.to_args_string(), "ght -D=5 -B=0.5 -LP=0 -SP=0.3 -HP=1");
    }

    #[test]
    fn with_clipmode() {
        let cmd = Ght::builder(5u8).clipmode(ClipMode::ReScale).build();
        assert_eq!(cmd.to_args_string(), "ght -D=5 -clipmode=rescale");
    }

    #[test]
    fn with_weight_human() {
        let cmd = Ght::builder(5u8).weight(ClipWeight::Human).build();
        assert_eq!(cmd.to_args_string(), "ght -D=5 -human");
    }

    #[test]
    fn with_weight_saturation() {
        let cmd = Ght::builder(5u8).weight(ClipWeight::Saturation).build();
        assert_eq!(cmd.to_args_string(), "ght -D=5 -sat");
    }

    #[test]
    fn with_channels() {
        let cmd = Ght::builder(5u8).channels(Channels::RG).build();
        assert_eq!(cmd.to_args_string(), "ght -D=5 RG");
    }

    #[test]
    fn all_options() {
        let cmd = Ght::builder(8u8)
            .focal_point(2.0f32)
            .low_point(0.1f32)
            .symetry_point(0.2f32)
            .high_point(0.9f32)
            .clipmode(ClipMode::Clip)
            .weight(ClipWeight::Even)
            .channels(Channels::GB)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "ght -D=8 -B=2 -LP=0.1 -SP=0.2 -HP=0.9 -clipmode=clip -even GB"
        );
    }
}
