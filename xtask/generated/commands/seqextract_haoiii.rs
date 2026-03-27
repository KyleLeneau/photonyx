use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqextract_HaOIII sequencename [-resample=]
/// ```
///
/// Same command as EXTRACT_HAOIII but for the sequence **sequencename**.
///
/// The output sequences names start with the prefixes "Ha\_" and "OIII\_"
///
#[derive(Builder)]
pub struct SeqextractHaOIII {}

impl Command for SeqextractHaOIII {
    fn name() -> &'static str {
        "seqextract_HaOIII"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
