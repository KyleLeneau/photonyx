use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// cd directory
/// ```
///
/// Sets the new current working directory.
///
/// The argument **directory** can contain the ~ token, expanded as the home directory, directories with spaces in the name can be protected using single or double quotes
///
#[derive(Builder)]
pub struct Cd {}

impl Command for Cd {
    fn name() -> &'static str {
        "cd"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
