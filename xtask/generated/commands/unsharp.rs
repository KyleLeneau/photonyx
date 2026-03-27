use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// unsharp sigma multi
/// ```
///
/// Applies an unsharp mask, actually a Gaussian filtered image with sigma **sigma** and a blend with the parameter **amount** used as such: out = in \* (1 + amount) + filtered \* (-amount).
///
/// See also GAUSS, the same without blending
///
/// Links: :ref:`gauss <gauss>`
///
#[derive(Builder)]
pub struct Unsharp {}

impl Command for Unsharp {
    fn name() -> &'static str {
        "unsharp"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
