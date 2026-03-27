use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// setcpu number
/// ```
///
/// Defines the number of processing threads used for calculation.
///
/// Can be as high as the number of virtual threads existing on the system, which is the number of CPU cores or twice this number if hyperthreading (Intel HT) is available. The default value is the maximum number of threads available, so this should mostly be used to limit processing power. This is reset on every Siril run. See also SETMEM
///
/// Links: :ref:`setmem <setmem>`
///
#[derive(Builder)]
pub struct Setcpu {}

impl Command for Setcpu {
    fn name() -> &'static str {
        "setcpu"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
