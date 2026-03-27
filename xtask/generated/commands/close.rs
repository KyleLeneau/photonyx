use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// close
/// ```
///
/// Properly closes the opened image and the opened sequence, if any
///
#[derive(Builder)]
pub struct Close {}

impl Command for Close {
    fn name() -> &'static str {
        "close"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
