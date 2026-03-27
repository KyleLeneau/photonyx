use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// cdg
/// ```
///
/// Returns the coordinates of the center of gravity of the image. Only pixels with values above 15.7% of max ADU and having four neighbors filling the same condition are used to compute it, and it is computed only if there are at least 50 of them
///
#[derive(Builder)]
pub struct Cdg {}

impl Command for Cdg {
    fn name() -> &'static str {
        "cdg"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
