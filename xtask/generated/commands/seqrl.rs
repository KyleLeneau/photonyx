use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqrl sequencename [-loadpsf=] [-alpha=] [-iters=] [-stop=] [-gdstep=] [-tv] [-fh] [-mul]
/// ```
///
/// The same as the RL command, but applies to a sequence which must be specified as the first argument
///
/// Links: :ref:`rl <rl>`
///
#[derive(Builder)]
pub struct Seqrl {}

impl Command for Seqrl {
    fn name() -> &'static str {
        "seqrl"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
