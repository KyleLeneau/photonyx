use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// ffti modulus phase
/// ```
///
/// Retrieves corrected image applying an inverse transformation. The **modulus** and **phase** arguments are the input file names, the result will be the new loaded image
///
#[derive(Builder)]
pub struct Ffti {}

impl Command for Ffti {
    fn name() -> &'static str {
        "ffti"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
