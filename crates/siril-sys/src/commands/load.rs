#![allow(async_fn_in_trait)]
use std::path::PathBuf;

use bon::Builder;

use crate::{
    Siril,
    commands::{Argument, Command},
    message::SirilError,
};

/// ```text
/// load filename[.ext]
/// ```
///
/// Loads the image **filename** from the current working directory, which becomes the 'currently loaded image' used in many of the single-image commands.
/// It first attempts to load **filename**, then **filename**.fit, **filename**.fits and finally all supported formats.
/// This scheme is applicable to every Siril command that involves reading files
///
#[derive(Builder)]
pub struct Load {
    #[builder(start_fn, into)]
    filename: String,
}

impl Command for Load {
    fn name() -> &'static str {
        "load"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(self.filename.clone())]
    }
}

pub trait LoadExt {
    async fn load_path(&mut self, path: PathBuf) -> Result<(), SirilError>;
}

impl LoadExt for Siril {
    async fn load_path(&mut self, path: PathBuf) -> Result<(), SirilError> {
        let cmd = Load::builder(path.display().to_string()).build();
        self.execute(&cmd).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_filename() {
        let cmd = Load::builder("light.fit").build();
        assert_eq!(cmd.to_args_string(), "load light.fit");
    }

    #[test]
    fn filename_with_spaces_is_quoted() {
        let cmd = Load::builder("my light.fit").build();
        assert_eq!(cmd.to_args_string(), "load 'my light.fit'");
    }
}
