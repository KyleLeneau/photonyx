use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqfindstar sequencename [-layer=] [-maxstars=]
/// ```
///
/// Same command as FINDSTAR but for the sequence **sequencename**.
///
/// The option **-out=** is not available for this process as all the star list files are saved with the default name *seqname_seqnb.lst*
///
/// Links: :ref:`findstar <findstar>`
///
#[derive(Builder)]
pub struct Seqfindstar {}

impl Command for Seqfindstar {
    fn name() -> &'static str {
        "seqfindstar"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
