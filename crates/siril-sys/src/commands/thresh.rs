use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// thresh lo hi
/// ```
///
/// Replaces values below **level** in the loaded image with **level**
///
#[derive(Builder)]
pub struct Thresh {
    #[builder(start_fn)]
    low: u8,
    #[builder(start_fn)]
    high: u8,
}

impl Command for Thresh {
    fn name() -> &'static str {
        "thresh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.low.to_string()),
            Argument::positional(self.high.to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic() {
        let cmd = Thresh::builder(10, 200).build();
        assert_eq!(cmd.to_args_string(), "thresh 10 200");
    }

    #[test]
    fn zero_low() {
        let cmd = Thresh::builder(0, 255).build();
        assert_eq!(cmd.to_args_string(), "thresh 0 255");
    }
}
