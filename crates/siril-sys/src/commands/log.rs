use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// log
/// ```
///
/// Computes and applies a logarithmic scale to the loaded image, using the following formula: log(1 - (value - min) / (max - min)), with min and max being the minimum and maximum pixel value for the channel
///
#[derive(Builder)]
pub struct Log {}

impl Command for Log {
    fn name() -> &'static str {
        "log"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args_renders_command_name() {
        let cmd = Log::builder().build();
        assert_eq!(cmd.to_args_string(), "log");
    }
}
