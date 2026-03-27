use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// invmodasinh -D= [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels]
/// ```
///
/// Inverts a modified arcsinh stretch. It provides the inverse transformation of MODASINH, if provided with the same parameters, undoes a MODASINH command, possibly returning to a linear image. It can also work the same way as MODASINH but for images in negative
///
/// Links: :ref:`modasinh <modasinh>`
///
#[derive(Builder)]
pub struct Invmodasinh {}

impl Command for Invmodasinh {
    fn name() -> &'static str {
        "invmodasinh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
