use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// rotate degree [-nocrop] [-interp=] [-noclamp]
/// ```
///
/// Rotates the loaded image by an angle of **degree** value. The option **-nocrop** can be added to avoid cropping to the image size (black borders will be added).
///
/// Note: if a selection is active, i.e. by using a command \`boxselect\` before \`rotate\`, the resulting image will be a rotated crop. In this particular case, the option **-nocrop** will be ignored if passed.
///
/// The pixel interpolation method can be specified with the **-interp=** argument followed by one of the methods in the list **no**\ [ne], **ne**\ [arest], **cu**\ [bic], **la**\ [nczos4], **li**\ [near], **ar**\ [ea]}. If **none** is passed, the transformation is forced to shift and a pixel-wise shift is applied to each image without any interpolation.
/// Clamping of the bicubic and lanczos4 interpolation methods is the default, to avoid artefacts, but can be disabled with the **-noclamp** argument
///
#[derive(Builder)]
pub struct Rotate {}

impl Command for Rotate {
    fn name() -> &'static str {
        "rotate"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
