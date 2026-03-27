use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// iadd filename
/// ```
///
/// Adds the image **filename** to the loaded image.
/// Result will be in 32 bits per channel if allowed in the preferences
///
#[derive(Builder)]
pub struct Iadd {}

impl Command for Iadd {
    fn name() -> &'static str {
        "iadd"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
