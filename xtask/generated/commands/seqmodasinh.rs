use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqmodasinh sequence -D= [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels] [-prefix=]
/// ```
///
/// Same command as MODASINH but the sequence must be specified as the first argument. In addition, the optional argument **-prefix=** can be used to set a custom prefix
///
/// Links: :ref:`modasinh <modasinh>`
///
#[derive(Builder)]
pub struct Seqmodasinh {}

impl Command for Seqmodasinh {
    fn name() -> &'static str {
        "seqmodasinh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
