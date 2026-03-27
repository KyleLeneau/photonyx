use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// gauss sigma
/// ```
///
/// Applies to the loaded image a Gaussian blur with the given **sigma**.
///
/// See also UNSHARP, the same with a blending parameter
///
/// Links: :ref:`unsharp <unsharp>`
///
#[derive(Builder)]
pub struct Gauss {}

impl Command for Gauss {
    fn name() -> &'static str {
        "gauss"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
