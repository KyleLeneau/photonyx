use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// nozero level
/// ```
///
/// Replaces null values by **level** values. Useful before an idiv or fdiv operation, mostly for 16-bit images
///
#[derive(Builder)]
pub struct Nozero {}

impl Command for Nozero {
    fn name() -> &'static str {
        "nozero"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
