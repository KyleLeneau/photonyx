use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// sequpdate_key sequencename key value [keycomment]
/// sequpdate_key sequencename -delete key
/// sequpdate_key sequencename -modify key newkey
/// sequpdate_key sequencename -comment comment
/// ```
///
/// Same command as UPDATE_KEY but for the sequence **sequencename**. However, this command won't work on SER sequence
///
/// Links: :ref:`update_key <update_key>`
///
#[derive(Builder)]
pub struct SequpdateKey {}

impl Command for SequpdateKey {
    fn name() -> &'static str {
        "sequpdate_key"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
