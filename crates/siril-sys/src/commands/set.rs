#![allow(async_fn_in_trait)]
use crate::{
    Siril, SirilSetting,
    commands::{Argument, Command},
    message::SirilError,
};
use bon::Builder;
use std::path::PathBuf;

/// ```text
/// set { -import=inifilepath | variable=value }
/// ```
///
/// Updates a setting value, using its variable name, with the given value, or a set of values using an existing ini file with **-import=** option.
/// See GET to get values or the list of variables
///
/// Links: :ref:`get <get>`
///
#[derive(Builder)]
pub struct Set {
    #[builder(start_fn)]
    method: Method,
}

pub enum Method {
    Import(String),
    Var(SirilSetting, String),
}

impl Command for Set {
    fn name() -> &'static str {
        "set"
    }

    fn args(&self) -> Vec<Argument> {
        match &self.method {
            Method::Import(file) => vec![Argument::option("import".to_string(), Some(file))],
            Method::Var(setting, value) => {
                vec![Argument::positional(format!("{}={}", setting, value))]
            }
        }
    }
}

pub trait SetExt {
    async fn set(
        &mut self,
        setting: SirilSetting,
        value: impl std::fmt::Display,
    ) -> Result<(), SirilError>;
    async fn set_import(&mut self, ini_file: PathBuf) -> Result<(), SirilError>;
}

impl SetExt for Siril {
    async fn set(
        &mut self,
        setting: SirilSetting,
        value: impl std::fmt::Display,
    ) -> Result<(), SirilError> {
        let method = Method::Var(setting, value.to_string());
        let cmd = Set::builder(method).build();
        self.execute(&cmd).await?;
        Ok(())
    }

    async fn set_import(&mut self, ini_file: PathBuf) -> Result<(), SirilError> {
        let method = Method::Import(ini_file.display().to_string());
        let cmd = Set::builder(method).build();
        self.execute(&cmd).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_method() {
        let cmd = Set::builder(Method::Import("/path/to/config.ini".into())).build();
        assert_eq!(cmd.to_args_string(), "set -import=/path/to/config.ini");
    }

    #[test]
    fn import_path_with_spaces_is_quoted() {
        let cmd = Set::builder(Method::Import("/my path/config.ini".into())).build();
        assert_eq!(cmd.to_args_string(), "set '-import=/my path/config.ini'");
    }

    #[test]
    fn var_method_extension() {
        let cmd = Set::builder(Method::Var(SirilSetting::Extension, "fits".into())).build();
        assert_eq!(cmd.to_args_string(), "set core.extension=fits");
    }

    #[test]
    fn var_method_force_16bit() {
        let cmd = Set::builder(Method::Var(SirilSetting::Force16Bit, "1".into())).build();
        assert_eq!(cmd.to_args_string(), "set core.force_16bit=1");
    }
}
