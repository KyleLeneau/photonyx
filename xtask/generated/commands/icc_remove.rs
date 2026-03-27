use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// icc_remove
/// ```
///
/// Removes the ICC profile from the current image, if it has one
///
#[derive(Builder)]
pub struct IccRemove {}

impl Command for IccRemove {
    fn name() -> &'static str {
        "icc_remove"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
