use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// online
/// ```
///
/// Sets Siril to online mode. In this mode networking functions such as remote catalogue lookups, update of git repositories etc. is allowed
///
#[derive(Builder)]
pub struct Online {}

impl Command for Online {
    fn name() -> &'static str {
        "online"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
