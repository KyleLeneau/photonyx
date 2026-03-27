use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// setext extension
/// ```
///
/// Sets the extension used and recognized by sequences.
///
/// The argument **extension** can be "fit", "fts" or "fits"
///
#[derive(Builder)]
pub struct Setext {}

impl Command for Setext {
    fn name() -> &'static str {
        "setext"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
