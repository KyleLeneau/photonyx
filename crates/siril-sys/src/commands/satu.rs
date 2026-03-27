use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// satu amount [background_factor [hue_range_index]]
/// ```
///
/// Enhances the color saturation of the loaded image. Try iteratively to obtain best results.
/// **amount** can be a positive number to increase color saturation, negative to decrease it, 0 would do nothing, 1 would increase it by 100%
/// **background_factor** is a factor to (median + sigma) used to set a threshold for which only pixels above it would be modified. This allows background noise to not be color saturated, if chosen carefully. Defaults to 1. Setting 0 disables the threshold.
/// **hue_range_index** can be [0, 6], meaning: 0 for pink to orange, 1 for orange to yellow, 2 for yellow to cyan, 3 for cyan, 4 for cyan to magenta, 5 for magenta to pink, 6 for all (default)
///
#[derive(Builder)]
pub struct Satu {}

impl Command for Satu {
    fn name() -> &'static str {
        "satu"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
