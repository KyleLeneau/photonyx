use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqsplit_cfa sequencename [-prefix=]
/// ```
///
/// Same command as SPLIT_CFA but for the sequence **sequencename**.
///
/// The output sequences names start with the prefix "CFA\_" and a number unless otherwise specified with **-prefix=** option.
/// *Limitation:* the sequence always outputs a sequence of FITS files, no matter the type of input sequence
///
/// Links: :ref:`split_cfa <split_cfa>`
///
#[derive(Builder)]
pub struct SeqsplitCfa {
    #[builder(start_fn, into)]
    sequence: String,
    #[builder(into)]
    prefix: Option<String>,
}

impl Command for SeqsplitCfa {
    fn name() -> &'static str {
        "seqsplit_cfa"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.sequence),
            Argument::option("prefix", self.prefix.as_deref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_only() {
        let cmd = SeqsplitCfa::builder("lights").build();
        assert_eq!(cmd.to_args_string(), "seqsplit_cfa lights");
    }

    #[test]
    fn custom_prefix() {
        let cmd = SeqsplitCfa::builder("lights").prefix("CFA2_").build();
        assert_eq!(cmd.to_args_string(), "seqsplit_cfa lights -prefix=CFA2_");
    }
}
