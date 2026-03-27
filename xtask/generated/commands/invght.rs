use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// invght -D= [-B=] [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels]
/// ```
///
/// Inverts a generalised hyperbolic stretch. It provides the inverse transformation of GHT, if provided with the same parameters, undoes a GHT command, possibly returning to a linear image. It can also work the same way as GHT but for images in negative
///
/// Links: :ref:`ght <ght>`
///
#[derive(Builder)]
pub struct Invght {}

impl Command for Invght {
    fn name() -> &'static str {
        "invght"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
