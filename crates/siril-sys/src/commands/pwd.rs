#![allow(async_fn_in_trait)]
use crate::{
    Siril,
    commands::{Argument, Command},
    message::SirilError,
};
use bon::Builder;
use std::path::PathBuf;

/// ```text
/// pwd
/// ```
///
/// Prints the current working directory
///
#[derive(Builder)]
pub struct Pwd {}

impl Command for Pwd {
    fn name() -> &'static str {
        "pwd"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

pub trait PwdExt {
    async fn current_directory(&mut self) -> Result<PathBuf, SirilError>;
}

impl PwdExt for Siril {
    async fn current_directory(&mut self) -> Result<PathBuf, SirilError> {
        let cmd = Pwd::builder().build();
        let results = self.execute(&cmd).await?;
        let path = results
            .iter()
            .find_map(|line| line.strip_prefix("Current working directory: '"))
            .and_then(|s| s.strip_suffix('\''))
            .map(PathBuf::from)
            .ok_or_else(|| SirilError::ParseError("pwd output not found".into()))?;
        Ok(path)
    }
}

// TODO: Implement Tests
