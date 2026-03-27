use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// split_cfa
/// ```
///
/// Splits the loaded CFA image into four distinct files (one for each channel) and saves them in files
///
#[derive(Builder)]
pub struct SplitCfa {}

impl Command for SplitCfa {
    fn name() -> &'static str {
        "split_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
