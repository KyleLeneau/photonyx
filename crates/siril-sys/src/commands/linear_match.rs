use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// linear_match reference low high
/// ```
///
/// Computes and applies a linear function between a **reference** image and the loaded image.
///
/// The algorithm will ignore all reference pixels whose values are outside of the [**low**, **high**] range
///
#[derive(Builder)]
pub struct LinearMatch {}

impl Command for LinearMatch {
    fn name() -> &'static str {
        "linear_match"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
