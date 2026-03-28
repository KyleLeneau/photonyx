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
    Single(String),
}

impl From<Method> for Argument {
    fn from(value: Method) -> Self {
        match value {
            Method::AllNames => Argument::flag("a".to_string()),
            Method::AllDetails => Argument::flag("A".to_string()),
            Method::Single(var) => Argument::positional(var),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_names_flag() {
        let cmd = Get::builder(Method::AllNames).build();
        assert_eq!(cmd.to_args_string(), "get -a");
    }

    #[test]
    fn all_details_flag() {
        let cmd = Get::builder(Method::AllDetails).build();
        assert_eq!(cmd.to_args_string(), "get -A");
    }

    #[test]
    fn single_variable() {
        let cmd = Get::builder(Method::Single("core.mem".to_string())).build();
        assert_eq!(cmd.to_args_string(), "get core.mem");
    }

    #[test]
    fn single_variable_with_spaces_is_quoted() {
        let cmd = Get::builder(Method::Single("my var".to_string())).build();
        assert_eq!(cmd.to_args_string(), "get 'my var'");
    }
}
