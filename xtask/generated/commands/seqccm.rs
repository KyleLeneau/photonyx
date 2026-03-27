use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqccm sequencename [-prefix=]
/// ```
///
/// Same command as CCM but for the the sequence **sequencename**. Only selected images in the sequence are processed.
///
/// The output sequence name starts with the prefix "ccm" unless otherwise specified with option **-prefix=**
///
/// Links: :ref:`ccm <ccm>`
///
#[derive(Builder)]
pub struct Seqccm {}

impl Command for Seqccm {
    fn name() -> &'static str {
        "seqccm"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
