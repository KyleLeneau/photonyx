use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqcosme_cfa sequencename [filename].lst [-prefix=]
/// ```
///
/// Same command as COSME_CFA but for the the sequence **sequencename**. Only selected images in the sequence are processed.
///
/// The output sequence name starts with the prefix "cosme\_" unless otherwise specified with option **-prefix=**
///
/// Links: :ref:`cosme_cfa <cosme_cfa>`
///
#[derive(Builder)]
pub struct SeqcosmeCfa {}

impl Command for SeqcosmeCfa {
    fn name() -> &'static str {
        "seqcosme_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
