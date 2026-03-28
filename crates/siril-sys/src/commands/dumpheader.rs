use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// dumpheader
/// ```
///
/// Dumps the FITS header of the loaded image in the console
///
#[derive(Builder)]
pub struct Dumpheader {}

impl Command for Dumpheader {
    fn name() -> &'static str {
        "dumpheader"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
// TODO: Implement Tests
