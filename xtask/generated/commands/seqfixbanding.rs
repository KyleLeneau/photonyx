use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqfixbanding sequencename amount sigma [-prefix=] [-vertical]
/// ```
///
/// Same command as FIXBANDING but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "unband\_" unless otherwise specified with **-prefix=** option
///
/// Links: :ref:`fixbanding <fixbanding>`
///
#[derive(Builder)]
pub struct Seqfixbanding {}

impl Command for Seqfixbanding {
    fn name() -> &'static str {
        "seqfixbanding"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
