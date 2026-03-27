use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqght sequence -D= [-B=] [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels] [-prefix=]
/// ```
///
/// Same command as GHT but the sequence must be specified as the first argument. In addition, the optional argument **-prefix=** can be used to set a custom prefix
///
/// Links: :ref:`ght <ght>`
///
#[derive(Builder)]
pub struct Seqght {}

impl Command for Seqght {
    fn name() -> &'static str {
        "seqght"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
