use bon::Builder;

use crate::{
    LimitOption,
    commands::{Argument, Command},
};

/// ```text
/// limit { -clip | -posrescale | -rescale }
/// ```
///
/// Limits pixel values in 32-bit images to the range 0.0 to 1.0. This command does not apply to 16-bit images as there cannot be out-of-range values. Range limiting can be done in one of the following ways:
///
/// **-clip**: this option simply clips all negative pixels to 0.0 and all pixels with a value > 1.0 to 1.0.
/// **-posrescale**: this option scales all positive pixel values so that the maximum value is 1.0, clipping any negative pixels to 0.0. For 3-channel images the same scaling factor is applied to all channels. If the maximum pixel value is already <= 1.0 negative pixels will still be clipped but no scaling factor will be applied to positive pixels.
/// **-rescale**: using this option, if there are any negative pixel values the image will have a constant value added to all pixel values so that the minimum value is 0.0. Then if the maximum pixel value is > 1.0, a scaling factor is applied so that the maximum pixel value is scaled to 1.0.
///
/// Note that if there are one or more extreme outliers (for example as a result of bad pixels) the **-rescale** and **-posrescale** options may produce an unexpected result. This can be mitigated by applying cosmetic correction to the image first
///
#[derive(Builder)]
pub struct Limit {
    #[builder(start_fn)]
    limit: LimitOption,
}

impl Command for Limit {
    fn name() -> &'static str {
        "limit"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::flag_option(self.limit.to_string(), true)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LimitOption;

    #[test]
    fn clip() {
        let cmd = Limit::builder(LimitOption::Clip).build();
        assert_eq!(cmd.to_args_string(), "limit -clip");
    }

    #[test]
    fn pos_rescale() {
        let cmd = Limit::builder(LimitOption::PosRescale).build();
        assert_eq!(cmd.to_args_string(), "limit -posrescale");
    }

    #[test]
    fn rescale() {
        let cmd = Limit::builder(LimitOption::Rescale).build();
        assert_eq!(cmd.to_args_string(), "limit -rescale");
    }
}
