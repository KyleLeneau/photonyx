use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// pm "expression" [-rescale [low] [high]] [-nosum]
/// ```
///
/// This command evaluates the expression given in argument as in PixelMath tool. The full expression must be between double quotes and variables (that are image names, without extension, located in the working directory in that case) must be surrounded by the token $, e.g. "$image1$ \* 0.5 + $image2$ \* 0.5". A maximum of 10 images can be used in the expression.
/// Image can be rescaled with the option **-rescale** followed by **low** and **high** values in the range [0, 1]. If no low and high values are provided, default values are set to 0 and 1. Another optional argument, **-nosum** tells Siril not to sum exposure times. This impacts FITS keywords such as LIVETIME and STACKCNT
///
#[derive(Builder)]
pub struct Pm {}

impl Command for Pm {
    fn name() -> &'static str {
        "pm"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
