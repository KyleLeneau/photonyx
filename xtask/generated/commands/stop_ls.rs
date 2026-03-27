use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// stop_ls
/// ```
///
/// Stops the live stacking session. Only possible after START_LS
///
/// Links: :ref:`start_ls <start_ls>`
///
#[derive(Builder)]
pub struct StopLs {}

impl Command for StopLs {
    fn name() -> &'static str {
        "stop_ls"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
