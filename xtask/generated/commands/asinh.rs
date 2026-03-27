use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// asinh [-human] stretch { [offset] [-clipmode=] }
/// ```
///
/// Stretches the image to show faint objects using an hyperbolic arcsin transformation. The mandatory argument **stretch**, typically between 1 and 1000, will give the strength of the stretch. The black point can be offset by providing an optional **offset** argument in the normalized pixel value of [0, 1]. Finally the option **-human** enables using human eye luminous efficiency weights to compute the luminance used to compute the stretch value for each pixel, instead of the simple mean of the channels pixel values. This stretch method preserves lightness from the L\*a\*b\* color space. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Asinh {}

impl Command for Asinh {
    fn name() -> &'static str {
        "asinh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
