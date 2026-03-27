use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// set { -import=inifilepath | variable=value }
/// ```
///
/// Updates a setting value, using its variable name, with the given value, or a set of values using an existing ini file with **-import=** option.
/// See GET to get values or the list of variables
///
/// Links: :ref:`get <get>`
///
#[derive(Builder)]
pub struct Set {}

impl Command for Set {
    fn name() -> &'static str {
        "set"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
