use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqextract_Ha sequencename [-prefix=] [-upscale]
/// ```
///
/// Same command as EXTRACT_HA but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "Ha\_" unless otherwise specified with option **-prefix=**
///
#[derive(Builder)]
pub struct SeqextractHa {}

impl Command for SeqextractHa {
    fn name() -> &'static str {
        "seqextract_Ha"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
