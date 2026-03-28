use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// exit
/// ```
///
/// Quits the application
///
#[derive(Builder)]
pub struct Exit {}

impl Command for Exit {
    fn name() -> &'static str {
        "exit"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
// TODO: Implement Tests
