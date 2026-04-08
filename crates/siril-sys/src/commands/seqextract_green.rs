use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqextract_Green sequencename [-prefix=]
/// ```
///
/// Same command as EXTRACT_GREEN but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "Green\_" unless otherwise specified with option **-prefix=**
///
#[derive(Builder)]
pub struct SeqextractGreen {}

impl Command for SeqextractGreen {
    fn name() -> &'static str {
        "seqextract_Green"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
