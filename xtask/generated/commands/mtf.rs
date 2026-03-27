use bon::Builder;

use crate::commands::{Argument, Command};

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
pub struct Mtf {}

impl Command for Mtf {
    fn name() -> &'static str {
        "mtf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
