use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// find_cosme_cfa cold_sigma hot_sigma
/// ```
///
/// Same command as FIND_COSME but for CFA images
///
/// Links: :ref:`find_cosme <find_cosme>`
///
#[derive(Builder)]
pub struct FindCosmeCfa {}

impl Command for FindCosmeCfa {
    fn name() -> &'static str {
        "find_cosme_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
