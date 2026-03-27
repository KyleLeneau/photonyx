use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// getref sequencename
/// ```
///
/// Prints information about the reference image of the sequence given in argument. First image has index 0
///
#[derive(Builder)]
pub struct Getref {}

impl Command for Getref {
    fn name() -> &'static str {
        "getref"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
