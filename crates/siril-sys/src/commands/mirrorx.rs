#![allow(async_fn_in_trait)]
use bon::Builder;

use crate::{
    Siril,
    commands::{Argument, Command},
    message::SirilError,
};

/// ```text
/// mirrorx [-bottomup]
/// ```
///
/// Flips the loaded image about the horizontal axis. Option **-bottomup** will only flip it if it's not already bottom-up
///
#[derive(Builder)]
pub struct Mirrorx {
    #[builder(default = true)]
    bottom_up: bool,
}

impl Command for Mirrorx {
    fn name() -> &'static str {
        "mirrorx"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::flag_option("bottomup", self.bottom_up)]
    }
}

pub trait MirrorxExt {
    async fn mirrorx(&mut self, bottom_up: bool) -> Result<(), SirilError>;
}

impl MirrorxExt for Siril {
    async fn mirrorx(&mut self, bottom_up: bool) -> Result<(), SirilError> {
        let cmd = Mirrorx::builder().bottom_up(bottom_up).build();
        self.execute(&cmd).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_includes_bottomup_flag() {
        let cmd = Mirrorx::builder().build();
        assert_eq!(cmd.to_args_string(), "mirrorx -bottomup");
    }

    #[test]
    fn bottomup_false_omits_flag() {
        let cmd = Mirrorx::builder().bottom_up(false).build();
        assert_eq!(cmd.to_args_string(), "mirrorx");
    }

    #[test]
    fn bottomup_true_includes_flag() {
        let cmd = Mirrorx::builder().bottom_up(true).build();
        assert_eq!(cmd.to_args_string(), "mirrorx -bottomup");
    }
}
