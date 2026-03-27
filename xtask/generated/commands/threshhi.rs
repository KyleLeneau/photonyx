use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// threshi level
/// ```
///
/// Replaces values above **level** in the loaded image with **level**
///
#[derive(Builder)]
pub struct Threshhi {}

impl Command for Threshhi {
    fn name() -> &'static str {
        "threshhi"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
