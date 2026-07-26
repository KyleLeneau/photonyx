use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// entropy
/// ```
///
/// Computes the entropy of the loaded image on the displayed layer, only in the selected area if one has been selected or in the whole image. The entropy is one way of measuring the noise or the details in an image
///
#[derive(Builder)]
pub struct Entropy {}

impl Command for Entropy {
    fn name() -> &'static str {
        "entropy"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
