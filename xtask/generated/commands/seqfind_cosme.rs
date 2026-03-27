use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqfind_cosme sequencename cold_sigma hot_sigma [-prefix=]
/// ```
///
/// Same command as FIND_COSME but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "cc\_" unless otherwise specified with **-prefix=** option
///
/// Links: :ref:`find_cosme <find_cosme>`
///
#[derive(Builder)]
pub struct SeqfindCosme {}

impl Command for SeqfindCosme {
    fn name() -> &'static str {
        "seqfind_cosme"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
