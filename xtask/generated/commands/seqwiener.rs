use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// wiener sequencename [-loadpsf=] [-alpha=]
/// ```
///
/// The same as the **WIENER** command, but applies to a sequence which must be specified as the first argument
///
/// Links: :ref:`wiener <wiener>`
///
#[derive(Builder)]
pub struct Seqwiener {}

impl Command for Seqwiener {
    fn name() -> &'static str {
        "seqwiener"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
