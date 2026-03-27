use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// fdiv filename scalar
/// ```
///
/// Divides the loaded image by the image given in argument. The resulting image is multiplied by the value of the **scalar** argument. See also IDIV
///
/// Links: :ref:`idiv <idiv>`
///
#[derive(Builder)]
pub struct Fdiv {}

impl Command for Fdiv {
    fn name() -> &'static str {
        "fdiv"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
