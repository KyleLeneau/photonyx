#![allow(async_fn_in_trait)]
use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// setref sequencename image_number
/// ```
///
/// Sets the reference image of the sequence given in first argument. **image_number** is the sequential number of the image in the sequence, not the number in the filename, starting at 1
///
#[derive(Builder)]
pub struct Setref {
    #[builder(start_fn, into)]
    sequence: String,
    #[builder(start_fn)]
    image_number: i32,
}

impl Command for Setref {
    fn name() -> &'static str {
        "setref"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.sequence.clone()),
            Argument::positional(self.image_number.to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_and_image_number() {
        let cmd = Setref::builder("lights", 1).build();
        assert_eq!(cmd.to_args_string(), "setref lights 1");
    }

    #[test]
    fn image_number_not_one() {
        let cmd = Setref::builder("lights", 5).build();
        assert_eq!(cmd.to_args_string(), "setref lights 5");
    }

    #[test]
    fn sequence_with_spaces_is_quoted() {
        let cmd = Setref::builder("my lights", 3).build();
        assert_eq!(cmd.to_args_string(), "setref 'my lights' 3");
    }
}
