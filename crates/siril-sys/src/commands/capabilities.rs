use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// capabilities
/// ```
///
/// Lists Siril capabilities, based on compilation options and runtime
///
#[derive(Builder)]
pub struct Capabilities {}

impl Command for Capabilities {
    fn name() -> &'static str {
        "capabilities"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Implement Tests
