use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// fmul scalar
/// ```
///
/// Multiplies the loaded image by the **scalar** given in argument
///
#[derive(Builder)]
pub struct Fmul {}

impl Command for Fmul {
    fn name() -> &'static str {
        "fmul"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
