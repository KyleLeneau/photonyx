use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqtilt sequencename
/// ```
///
/// Same command as TILT but for the sequence **sequencename**. It generally gives better results
///
/// Links: :ref:`tilt <tilt>`
///
#[derive(Builder)]
pub struct Seqtilt {}

impl Command for Seqtilt {
    fn name() -> &'static str {
        "seqtilt"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
