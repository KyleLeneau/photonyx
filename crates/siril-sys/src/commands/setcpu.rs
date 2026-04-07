#![allow(async_fn_in_trait)]
use bon::Builder;

use crate::{
    Siril,
    commands::{Argument, Command},
    message::SirilError,
};

/// ```text
/// setcpu number
/// ```
///
/// Defines the number of processing threads used for calculation.
///
/// Can be as high as the number of virtual threads existing on the system, which is the number of CPU cores or twice this number if hyperthreading (Intel HT) is available. The default value is the maximum number of threads available, so this should mostly be used to limit processing power. This is reset on every Siril run. See also SETMEM
///
/// Links: :ref:`setmem <setmem>`
///
#[derive(Builder)]
pub struct Setcpu {
    #[builder(start_fn)]
    number: u8,
}

impl Command for Setcpu {
    fn name() -> &'static str {
        "setcpu"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(self.number.to_string())]
    }
}

pub trait SetcpuExt {
    async fn set_cpu_cores(&mut self, cores: u8) -> Result<(), SirilError>;
}

impl SetcpuExt for Siril {
    async fn set_cpu_cores(&mut self, cores: u8) -> Result<(), SirilError> {
        let cmd = Setcpu::builder(cores).build();
        self.execute(&cmd).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_thread() {
        let cmd = Setcpu::builder(1).build();
        assert_eq!(cmd.to_args_string(), "setcpu 1");
    }

    #[test]
    fn multiple_threads() {
        let cmd = Setcpu::builder(8).build();
        assert_eq!(cmd.to_args_string(), "setcpu 8");
    }
}
