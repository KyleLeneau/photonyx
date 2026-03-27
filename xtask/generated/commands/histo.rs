use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// histo channel (channel=0, 1, 2 with 0: red, 1: green, 2: blue)
/// ```
///
/// Calculates the histogram of the **layer** of the loaded image and produces file histo\_[channel name].dat in the working directory.
/// layer = 0, 1 or 2 with 0=red, 1=green and 2=blue
///
#[derive(Builder)]
pub struct Histo {}

impl Command for Histo {
    fn name() -> &'static str {
        "histo"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
