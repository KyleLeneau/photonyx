use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// threshi level
/// ```
///
/// Replaces values above **level** in the loaded image with **level**
///
#[derive(Builder)]
pub struct Threshhi {
    #[builder(start_fn)]
    level: u8,
}

impl Command for Threshhi {
    fn name() -> &'static str {
        "threshhi"
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
        let cmd = Threshhi::builder(200).build();
        assert_eq!(cmd.to_args_string(), "threshhi 200");
    }

    #[test]
    fn max_level() {
        let cmd = Threshhi::builder(255).build();
        assert_eq!(cmd.to_args_string(), "threshhi 255");
    }
}
