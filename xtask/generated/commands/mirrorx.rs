use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// mirrorx [-bottomup]
/// ```
///
/// Flips the loaded image about the horizontal axis. Option **-bottomup** will only flip it if it's not already bottom-up
///
#[derive(Builder)]
pub struct Mirrorx {}

impl Command for Mirrorx {
    fn name() -> &'static str {
        "mirrorx"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
