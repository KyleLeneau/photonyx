use bon::Builder;

use crate::{
    SirilSetting,
    commands::{Argument, Command},
};

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

// TODO: Implement Tests
