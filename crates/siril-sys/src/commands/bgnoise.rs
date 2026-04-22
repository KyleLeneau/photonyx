use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// bgnoise
/// ```
///
/// Returns the background noise level of the loaded image
///
/// For more information, see the :ref:`statistics documentation
/// <Statistics:Background noise>`
///
#[derive(Builder)]
pub struct Bgnoise {}

impl Command for Bgnoise {
    fn name() -> &'static str {
        "bgnoise"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
