use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// linstretch -BP= [-sat] [-clipmode=] [channels] [-clipmode=]
/// ```
///
/// Stretches the image linearly to a new black point BP.
/// The argument **[channels]** may optionally be used to specify the channels to apply the stretch to: this may be R, G, B, RG, RB or GB. The default is all channels.
/// Optionally the parameter **-sat** may be used to apply the linear stretch to the image saturation channel. This argument only works if all channels are selected. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Linstretch {}

impl Command for Linstretch {
    fn name() -> &'static str {
        "linstretch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
