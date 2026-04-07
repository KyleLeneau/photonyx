#![allow(async_fn_in_trait)]
use bon::Builder;

use crate::{
    Siril,
    commands::{Argument, Command},
    message::SirilError,
};

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

pub trait CapabilitiesExt {
    async fn capabilities(&mut self) -> Result<Vec<String>, SirilError>;
}

impl CapabilitiesExt for Siril {
    async fn capabilities(&mut self) -> Result<Vec<String>, SirilError> {
        let cmd = Capabilities::builder().build();
        self.execute(&cmd).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_renders_command_name() {
        let cmd = Capabilities::builder().build();
        assert_eq!(cmd.to_args_string(), "capabilities");
    }
}
