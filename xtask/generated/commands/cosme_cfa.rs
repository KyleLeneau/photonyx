use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// cosme_cfa [filename].lst
/// ```
///
/// Same function as COSME but applying to RAW CFA images
///
/// Links: :ref:`cosme <cosme>`
///
#[derive(Builder)]
pub struct CosmeCfa {}

impl Command for CosmeCfa {
    fn name() -> &'static str {
        "cosme_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
