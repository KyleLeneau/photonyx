use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// setref sequencename image_number
/// ```
///
/// Sets the reference image of the sequence given in first argument. **image_number** is the sequential number of the image in the sequence, not the number in the filename, starting at 1
///
#[derive(Builder)]
pub struct Setref {}

impl Command for Setref {
    fn name() -> &'static str {
        "setref"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
