use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqlinstretch sequence -BP= [channels] [-sat] [-prefix=]
/// ```
///
/// Same command as LINSTRETCH but the sequence must be specified as the first argument. In addition, the optional argument **-prefix=** can be used to set a custom prefix
///
/// Links: :ref:`linstretch <linstretch>`
///
#[derive(Builder)]
pub struct Seqlinstretch {}

impl Command for Seqlinstretch {
    fn name() -> &'static str {
        "seqlinstretch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
