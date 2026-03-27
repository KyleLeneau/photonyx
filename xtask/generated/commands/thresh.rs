use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// thresh lo hi
/// ```
///
/// Replaces values below **level** in the loaded image with **level**
///
#[derive(Builder)]
pub struct Thresh {}

impl Command for Thresh {
    fn name() -> &'static str {
        "thresh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
