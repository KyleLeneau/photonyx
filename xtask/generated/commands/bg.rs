use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// bg
/// ```
///
/// Returns the background level of the loaded image
///
#[derive(Builder)]
pub struct Bg {}

impl Command for Bg {
    fn name() -> &'static str {
        "bg"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
