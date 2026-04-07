use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// set32bits
/// ```
///
/// Allows images to be saved with 32 bits per channel on processing
///
#[derive(Builder)]
pub struct Set32bits {}

impl Command for Set32bits {
    fn name() -> &'static str {
        "set32bits"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args() {
        let cmd = Set32bits::builder().build();
        assert_eq!(cmd.to_args_string(), "set32bits");
    }
}
