use bon::Builder;
use std::path::PathBuf;

use crate::commands::{Argument, Command};

/// .. code-block:: text
///
///     cd directory
///
/// Sets the new current working directory.
///
/// The argument **directory** can contain the ~ token, expanded as the home directory, directories with spaces in the name can be protected using single or double quotes
#[derive(Builder)]
pub struct Cd {
    #[builder(start_fn)]
    directory: PathBuf,
}

impl Command for Cd {
    fn name() -> &'static str {
        "cd"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(self.directory.display().to_string())]
    }
}
