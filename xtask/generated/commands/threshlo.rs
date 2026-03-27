use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// threshlo level
/// ```
///
/// Replaces values below **level** in the loaded image with **level**
///
#[derive(Builder)]
pub struct Threshlo {}

impl Command for Threshlo {
    fn name() -> &'static str {
        "threshlo"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
