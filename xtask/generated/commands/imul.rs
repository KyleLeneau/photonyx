use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// imul filename
/// ```
///
/// Multiplies image **filename** by the loaded image.
/// Result will be in 32 bits per channel if allowed in the preferences
///
#[derive(Builder)]
pub struct Imul {}

impl Command for Imul {
    fn name() -> &'static str {
        "imul"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
