use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqinvght sequence -D= [-B=] [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels] [-prefix=]
/// ```
///
/// Same command as INVGHT but the sequence must be specified as the first argument. In addition, the optional argument **-prefix=** can be used to set a custom prefix
///
/// Links: :ref:`invght <invght>`
///
#[derive(Builder)]
pub struct Seqinvght {}

impl Command for Seqinvght {
    fn name() -> &'static str {
        "seqinvght"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
