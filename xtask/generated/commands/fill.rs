use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// fill value [x y width height]
/// ```
///
/// Fills the loaded image entirely or only the selection if there is one with pixels having the **value** intensity expressed in ADU
///
#[derive(Builder)]
pub struct Fill {}

impl Command for Fill {
    fn name() -> &'static str {
        "fill"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
