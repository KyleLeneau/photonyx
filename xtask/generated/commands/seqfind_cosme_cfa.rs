use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqfind_cosme_cfa sequencename cold_sigma hot_sigma [-prefix=]
/// ```
///
/// Same command as FIND_COSME_CFA but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "cc\_" unless otherwise specified with **-prefix=** option
///
/// Links: :ref:`find_cosme_cfa <find_cosme_cfa>`
///
#[derive(Builder)]
pub struct SeqfindCosmeCfa {}

impl Command for SeqfindCosmeCfa {
    fn name() -> &'static str {
        "seqfind_cosme_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
