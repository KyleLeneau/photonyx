use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqextract_Green sequencename [-prefix=]
/// ```
///
/// Same command as EXTRACT_GREEN but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "Green\_" unless otherwise specified with option **-prefix=**
///
#[derive(Builder)]
pub struct SeqextractGreen {
    #[builder(start_fn, into)]
    sequence: String,
    #[builder(into)]
    prefix: Option<String>,
}

impl Command for SeqextractGreen {
    fn name() -> &'static str {
        "seqextract_Green"
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
        let cmd = SeqextractGreen::builder("lights").build();
        assert_eq!(cmd.to_args_string(), "seqextract_Green lights");
    }

    #[test]
    fn custom_prefix() {
        let cmd = SeqextractGreen::builder("lights").prefix("Green2_").build();
        assert_eq!(
            cmd.to_args_string(),
            "seqextract_Green lights -prefix=Green2_"
        );
    }
}
