use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqcosme sequencename [filename].lst [-prefix=]
/// ```
///
/// Same command as COSME but for the the sequence **sequencename**. Only selected images in the sequence are processed.
///
/// The output sequence name starts with the prefix "cosme\_" unless otherwise specified with option **-prefix=**
///
/// Links: :ref:`cosme <cosme>`
///
#[derive(Builder)]
pub struct Seqcosme {}

impl Command for Seqcosme {
    fn name() -> &'static str {
        "seqcosme"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
