use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// rotatePi
/// ```
///
/// Rotates the loaded image of an angle of 180° around its center. This is equivalent to the command "ROTATE 180" or "ROTATE -180"
///
/// Links: :ref:`rotate <rotate>`
///
#[derive(Builder)]
pub struct RotatePi {}

impl Command for RotatePi {
    fn name() -> &'static str {
        "rotatePi"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
