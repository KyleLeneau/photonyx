use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// get { -a | -A | variable }
/// ```
///
/// Gets a value from the settings using its name, or list all with **-a** (name and value list) or with **-A** (detailed list)
///
/// See also SET to update values
///
/// Links: :ref:`set <set>`
///
#[derive(Builder)]
pub struct Get {
    #[builder(start_fn)]
    method: Method,
}

#[derive(Clone)]
pub enum Method {
    AllNames,
    AllDetails,
    Single(String)
}

impl From<Method> for Argument  {
    fn from(value: Method) -> Self {
        match value {
            Method::AllNames => Argument::Flag("-a".to_string(), Some(true)),
            Method::AllDetails => Argument::Flag("-A".to_string(), Some(true)),
            Method::Single(var) => Argument::Positional(var),
        }
    }
}

impl Command for Get {
    fn name() -> &'static str {
        "get"
    }

    fn args(&self) -> Vec<Argument> {
        vec![self.method.clone().into()]
    }
}
