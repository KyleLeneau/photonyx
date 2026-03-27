use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqstarnet sequencename [-stretch] [-upscale] [-stride=value] [-nostarmask]
/// ```
///
/// This command calls `Starnet++ <https://www.starnetastro.com/>`__ to remove stars from the sequence **sequencename**. See STARNET
///
/// Links: :ref:`starnet <starnet>`
///
#[derive(Builder)]
pub struct Seqstarnet {}

impl Command for Seqstarnet {
    fn name() -> &'static str {
        "seqstarnet"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
