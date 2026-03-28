use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// mirrory
/// ```
///
/// Flips the image about the vertical axis
///
#[derive(Builder)]
pub struct Mirrory {}

impl Command for Mirrory {
    fn name() -> &'static str {
        "mirrory"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
// TODO: Implement Tests
