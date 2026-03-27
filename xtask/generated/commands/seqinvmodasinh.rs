use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqinvmodasinh sequence -D= [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels] [-prefix=]
/// ```
///
/// Same command as INVMODASINH but the sequence must be specified as the first argument. In addition, the optional argument **-prefix=** can be used to set a custom prefix
///
/// Links: :ref:`invmodasinh <invmodasinh>`
///
#[derive(Builder)]
pub struct Seqinvmodasinh {}

impl Command for Seqinvmodasinh {
    fn name() -> &'static str {
        "seqinvmodasinh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
