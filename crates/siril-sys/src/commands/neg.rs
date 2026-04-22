use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// neg
/// ```
///
/// Changes pixel values of the currently loaded image to a negative view, like 1-value for 32 bits, 65535-value for 16 bits. This does not change the display mode
///
#[derive(Builder)]
pub struct Neg {}

impl Command for Neg {
    fn name() -> &'static str {
        "neg"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
