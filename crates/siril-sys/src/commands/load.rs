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

// TODO: Implement Tests
