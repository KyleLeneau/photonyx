use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// isub filename
/// ```
///
/// Subtracts the loaded image by the image **filename**.
/// Result will be in 32 bits per channel if allowed in the preferences, so capable of storing negative values. To clip negative value, use 16 bit mode or use the THRESHLO command
///
/// Links: :ref:`threshlo <threshlo>`
///
#[derive(Builder)]
pub struct Isub {}

impl Command for Isub {
    fn name() -> &'static str {
        "isub"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
