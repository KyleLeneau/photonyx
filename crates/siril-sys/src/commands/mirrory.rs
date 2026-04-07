#![allow(async_fn_in_trait)]
use bon::Builder;

use crate::{
    Siril,
    commands::{Argument, Command},
    message::SirilError,
};

/// ```text
/// mirrory
/// ```
///
/// Flips the image about the vertical axis
///
#[derive(Builder)]
pub struct Mirrory {}

impl Command for Mirrory {
    fn name() -> &'static str {
        "mirrory"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

pub trait MirroryxExt {
    async fn mirrory(&mut self) -> Result<(), SirilError>;
}

impl MirroryxExt for Siril {
    async fn mirrory(&mut self) -> Result<(), SirilError> {
        let cmd = Mirrory::builder().build();
        self.execute(&cmd).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_renders_command_name() {
        let cmd = Mirrory::builder().build();
        assert_eq!(cmd.to_args_string(), "mirrory");
    }
}
