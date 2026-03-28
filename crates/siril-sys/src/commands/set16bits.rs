use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// set16bits
/// ```
///
/// Forbids images to be saved with 32 bits per channel on processing, use 16 bits instead
///
#[derive(Builder)]
pub struct Set16bits {}

impl Command for Set16bits {
    fn name() -> &'static str {
        "set16bits"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
// TODO: Implement Tests
