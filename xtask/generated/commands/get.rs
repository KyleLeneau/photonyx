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
pub struct Get {}

impl Command for Get {
    fn name() -> &'static str {
        "get"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
