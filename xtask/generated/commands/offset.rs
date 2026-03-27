use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// offset value
/// ```
///
/// Adds the constant **value** (specified in ADU) to the current image. This constant can take a negative value.
///
/// In 16-bit mode, values of pixels that fall outside of [0, 65535] are clipped. In 32-bit mode, no clipping occurs
///
#[derive(Builder)]
pub struct Offset {}

impl Command for Offset {
    fn name() -> &'static str {
        "offset"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
