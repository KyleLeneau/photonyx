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
    #[builder(default)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_sequences_and_output() {
        let cmd = Merge::builder("seq1", "seq2", "merged").build();
        assert_eq!(cmd.to_args_string(), "merge seq1 seq2 merged");
    }

    #[test]
    fn extra_sequences_inserted_before_output() {
        let cmd = Merge::builder("seq1", "seq2", "merged")
            .extras(vec!["seq3".into(), "seq4".into()])
            .build();
        assert_eq!(cmd.to_args_string(), "merge seq1 seq2 seq3 seq4 merged");
    }

    #[test]
    fn sequence_with_spaces_is_quoted() {
        let cmd = Merge::builder("my seq1", "seq2", "out seq").build();
        assert_eq!(cmd.to_args_string(), "merge 'my seq1' seq2 'out seq'");
    }
}
