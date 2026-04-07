#![allow(async_fn_in_trait)]
use bon::Builder;

use crate::{
    Siril,
    commands::{Argument, Command},
    message::SirilError,
};

/// ```text
/// close
/// ```
///
/// Properly closes the opened image and the opened sequence, if any
///
#[derive(Builder)]
pub struct Close {}

impl Command for Close {
    fn name() -> &'static str {
        "close"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

pub trait CloseExt {
    async fn close(&mut self) -> Result<(), SirilError>;
}

impl CloseExt for Siril {
    async fn close(&mut self) -> Result<(), SirilError> {
        let cmd = Close::builder().build();
        self.execute(&cmd).await?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_renders_command_name() {
        let cmd = Close::builder().build();
        assert_eq!(cmd.to_args_string(), "close");
    }
}
