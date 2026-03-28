use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// merge sequence1 sequence2 [sequence3 ...] output_sequence
/// ```
///
/// Merges several sequences of the same type (FITS images, FITS sequence or SER) and same image properties into a new sequence with base name **newseq** created in the current working directory, with the same type. The input sequences can be in different directories, can specified either in absolute or relative path, with the exact .seq name or with only the base name with or without the trailing '\_'
///
#[derive(Builder)]
pub struct Merge {
    #[builder(start_fn, into)]
    sequence1: String,
    #[builder(start_fn, into)]
    sequence2: String,
    #[builder(start_fn, into)]
    output: String,
    extras: Vec<String>,
}

impl Command for Merge {
    fn name() -> &'static str {
        "merge"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![
            Argument::positional(self.sequence1.to_string()),
            Argument::positional(self.sequence2.to_string()),
        ];

        for extra in &self.extras {
            args.push(Argument::positional(extra));
        }

        args.push(Argument::positional(self.output.to_string()));
        args
    }
}

// TODO: Implement Tests
