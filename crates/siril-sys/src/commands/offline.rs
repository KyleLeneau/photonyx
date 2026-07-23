use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// offline
/// ```
///
/// Sets Siril to offline mode. In this mode networking functions such as remote catalogue lookups, update of git repositories etc. are unavailable. Cached data is still accessible
///
#[derive(Builder)]
pub struct Offline {}

impl Command for Offline {
    fn name() -> &'static str {
        "offline"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
