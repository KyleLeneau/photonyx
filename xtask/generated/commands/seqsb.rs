use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqsb sequencename [-loadpsf=] [-alpha=] [-iters=]
/// ```
///
/// The same as the SB command, but applies to a sequence which must be specified as the first argument
///
/// Links: :ref:`sb <sb>`
///
#[derive(Builder)]
pub struct Seqsb {}

impl Command for Seqsb {
    fn name() -> &'static str {
        "seqsb"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
