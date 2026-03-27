use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// idiv filename
/// ```
///
/// Divides the loaded image by the image **filename**.
/// Result will be in 32 bits per channel if allowed in the preferences.
///
/// See also FDIV
///
/// Links: :ref:`fdiv <fdiv>`
///
#[derive(Builder)]
pub struct Idiv {}

impl Command for Idiv {
    fn name() -> &'static str {
        "idiv"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
