use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqsplit_cfa sequencename [-prefix=]
/// ```
///
/// Same command as SPLIT_CFA but for the sequence **sequencename**.
///
/// The output sequences names start with the prefix "CFA\_" and a number unless otherwise specified with **-prefix=** option.
/// *Limitation:* the sequence always outputs a sequence of FITS files, no matter the type of input sequence
///
/// Links: :ref:`split_cfa <split_cfa>`
///
#[derive(Builder)]
pub struct SeqsplitCfa {}

impl Command for SeqsplitCfa {
    fn name() -> &'static str {
        "seqsplit_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
