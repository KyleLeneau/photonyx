#![allow(async_fn_in_trait)]
use bon::Builder;

use crate::{
    Siril,
    commands::{Argument, Command},
    message::SirilError,
};

/// ```text
/// dumpheader
/// ```
///
/// Dumps the FITS header of the loaded image in the console
///
#[derive(Builder)]
pub struct Dumpheader {}

impl Command for Dumpheader {
    fn name() -> &'static str {
        "dumpheader"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

pub trait DumpheaderExt {
    async fn dumpheader(&mut self) -> Result<Vec<String>, SirilError>;
}

impl DumpheaderExt for Siril {
    async fn dumpheader(&mut self) -> Result<Vec<String>, SirilError> {
        let cmd = Dumpheader::builder().build();
        self.execute(&cmd).await
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_renders_command_name() {
        let cmd = Dumpheader::builder().build();
        assert_eq!(cmd.to_args_string(), "dumpheader");
    }
}
