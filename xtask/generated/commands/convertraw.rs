use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// convertraw basename [-debayer] [-fitseq] [-ser] [-start=index] [-out=]
/// ```
///
/// Same as CONVERT but converts only DSLR RAW files found in the current working directory
///
/// Links: :ref:`convert <convert>`
///
#[derive(Builder)]
pub struct Convertraw {}

impl Command for Convertraw {
    fn name() -> &'static str {
        "convertraw"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
