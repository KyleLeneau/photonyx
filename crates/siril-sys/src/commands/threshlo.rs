use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// threshlo level
/// ```
///
/// Replaces values below **level** in the loaded image with **level**
///
#[derive(Builder)]
pub struct Threshlo {
    #[builder(start_fn)]
    level: u8,
}

impl Command for Threshlo {
    fn name() -> &'static str {
        "threshlo"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(self.level.to_string())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let cmd = Threshlo::builder(10).build();
        assert_eq!(cmd.to_args_string(), "threshlo 10");
    }

    #[test]
    fn zero_level() {
        let cmd = Threshlo::builder(0).build();
        assert_eq!(cmd.to_args_string(), "threshlo 0");
    }
}
