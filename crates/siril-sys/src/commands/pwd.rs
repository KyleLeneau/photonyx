use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// pwd
/// ```
///
/// Prints the current working directory
///
#[derive(Builder)]
pub struct Pwd {}

impl Command for Pwd {
    fn name() -> &'static str {
        "pwd"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
// TODO: Implement Tests
