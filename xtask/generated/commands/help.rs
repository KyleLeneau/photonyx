use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// help [command]
/// ```
///
/// Lists the available commands or help for one command
///
#[derive(Builder)]
pub struct Help {}

impl Command for Help {
    fn name() -> &'static str {
        "help"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
