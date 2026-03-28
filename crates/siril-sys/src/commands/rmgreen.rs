use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// rmgreen [-nopreserve] [type] [amount]
/// ```
///
/// Applies a chromatic noise reduction filter. It removes green tint in the current image. This filter is based on PixInsight's SCNR and it is also the same filter used by HLVG plugin in Photoshop.
/// Lightness is preserved by default but this can be disabled with the **-nopreserve** switch.
///
/// **Type** can take values 0 for average neutral, 1 for maximum neutral, 2 for maximum mask, 3 for additive mask, defaulting to 0. The last two can take an **amount** argument, a value between 0 and 1, defaulting to 1
///
#[derive(Builder)]
pub struct Rmgreen {}

impl Command for Rmgreen {
    fn name() -> &'static str {
        "rmgreen"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
// TODO: Implement Tests
