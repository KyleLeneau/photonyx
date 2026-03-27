use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// invmtf low mid high [channels]
/// ```
///
/// Inverts a midtones transfer function. It provides the inverse transformation of MTF, if provided with the same parameters, undoes a MTF command, possibly returning to a linear image. It can also work the same way as MTF but for images in negative
///
/// Links: :ref:`mtf <mtf>`
///
#[derive(Builder)]
pub struct Invmtf {}

impl Command for Invmtf {
    fn name() -> &'static str {
        "invmtf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
