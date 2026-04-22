use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// nozero level
/// ```
///
/// Replaces null values by **level** values. Useful before an idiv or fdiv operation, mostly for 16-bit images
///
#[derive(Builder)]
pub struct Nozero {
    #[builder(start_fn)]
    level: f32,
}

impl Command for Nozero {
    fn name() -> &'static str {
        "nozero"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(self.level.to_string())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_level() {
        let cmd = Nozero::builder(1.0_f32).build();
        assert_eq!(cmd.to_args_string(), "nozero 1");
    }

    #[test]
    fn with_fractional_level() {
        let cmd = Nozero::builder(0.001_f32).build();
        assert_eq!(cmd.to_args_string(), "nozero 0.001");
    }
}
