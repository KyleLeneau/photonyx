use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// merge sequence1 sequence2 [sequence3 ...] output_sequence
/// ```
///
/// Merges several sequences of the same type (FITS images, FITS sequence or SER) and same image properties into a new sequence with base name **newseq** created in the current working directory, with the same type. The input sequences can be in different directories, can specified either in absolute or relative path, with the exact .seq name or with only the base name with or without the trailing '\_'
///
#[derive(Builder)]
pub struct Merge {}

impl Command for Merge {
    fn name() -> &'static str {
        "merge"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
