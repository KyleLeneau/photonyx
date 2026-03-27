use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// getref sequencename
/// ```
///
/// Prints information about the reference image of the sequence given in argument. First image has index 0
///
#[derive(Builder)]
pub struct Getref {
    #[builder(start_fn)]
    sequence: String
}

impl Command for Getref {
    fn name() -> &'static str {
        "getref"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(self.sequence.to_string())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_sequence_name() {
        let cmd = Getref::builder("lights".to_string()).build();
        assert_eq!(cmd.to_args_string(), "getref lights");
    }

    #[test]
    fn sequence_name_with_spaces_is_quoted() {
        let cmd = Getref::builder("my lights".to_string()).build();
        assert_eq!(cmd.to_args_string(), "getref 'my lights'");
    }
}
