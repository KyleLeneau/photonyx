use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// autoghs [-linked] shadowsclip stretchamount [-b=] [-hp=] [-lp=] [-clipmode=]
/// ```
///
/// Application of the generalized hyperbolic stretch with a symmetry point SP defined as k.sigma from the median of each channel (the provided **shadowsclip** value is the k here and can be negative). By default, SP and the stretch are computed per channel; SP can be computed as a mean of image channels by passing **-linked**. The stretch amount **D** is provided in the second mandatory argument.
/// Implicit values of 13 for **B**, making it very focused on the SP brightness range, 0.7 for **HP**, 0 for **LP** are used but can be changed with the options of the same names. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Autoghs {}

impl Command for Autoghs {
    fn name() -> &'static str {
        "autoghs"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
