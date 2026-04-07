#![allow(async_fn_in_trait)]
use bon::Builder;

use crate::{
    Siril,
    commands::{Argument, Command},
    message::SirilError,
};

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

pub trait ExitExt {
    async fn exit(&mut self) -> Result<(), SirilError>;
}

impl ExitExt for Siril {
    async fn exit(&mut self) -> Result<(), SirilError> {
        let cmd = Exit::builder().build();
        self.execute(&cmd).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_renders_command_name() {
        let cmd = Exit::builder().build();
        assert_eq!(cmd.to_args_string(), "exit");
    }
}
