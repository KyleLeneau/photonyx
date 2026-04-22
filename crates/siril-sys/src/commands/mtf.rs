use bon::Builder;

use crate::{
    Channels,
    commands::{Argument, Command},
};

/// ```text
/// mtf low mid high [channels]
/// ```
///
/// Applies midtones transfer function to the current loaded image.
///
/// Three parameters are needed, **low**, **midtones** and **high** where midtones balance parameter defines a nonlinear histogram stretch in the [0,1] range. For an automatic determination of the parameters, see AUTOSTRETCH.
/// Optionally the parameter **[channels]** may be used to specify the channels to apply the stretch to: this may be R, G, B, RG, RB or GB. The default is all channels
///
/// Links: :ref:`autostretch <autostretch>`
///
#[derive(Builder)]
pub struct Mtf {
    #[builder(start_fn)]
    low: f32,
    #[builder(start_fn)]
    mid: f32,
    #[builder(start_fn)]
    high: f32,
    channels: Option<Channels>,
}

impl Command for Mtf {
    fn name() -> &'static str {
        "mtf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.low.to_string()),
            Argument::positional(self.mid.to_string()),
            Argument::positional(self.high.to_string()),
            Argument::positional_option(self.channels.as_ref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_channels_no_override() {
        let cmd = Mtf::builder(0.0_f32, 0.5_f32, 1.0_f32).build();
        assert_eq!(cmd.to_args_string(), "mtf 0 0.5 1");
    }

    #[test]
    fn with_channel_selection() {
        let cmd = Mtf::builder(0.0_f32, 0.5_f32, 1.0_f32)
            .channels(Channels::G)
            .build();
        assert_eq!(cmd.to_args_string(), "mtf 0 0.5 1 G");
    }

    #[test]
    fn with_multi_channel_selection() {
        let cmd = Mtf::builder(0.1_f32, 0.4_f32, 0.9_f32)
            .channels(Channels::RB)
            .build();
        assert_eq!(cmd.to_args_string(), "mtf 0.1 0.4 0.9 RB");
    }
}
