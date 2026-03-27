use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqmtf sequencename low mid high [channels] [-prefix=]
/// ```
///
/// Same command as MTF but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "mtf\_" unless otherwise specified with **-prefix=** option
///
/// Links: :ref:`mtf <mtf>`
///
#[derive(Builder)]
pub struct Seqmtf {}

impl Command for Seqmtf {
    fn name() -> &'static str {
        "seqmtf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
