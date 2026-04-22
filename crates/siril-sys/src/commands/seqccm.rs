use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqccm sequencename [-prefix=]
/// ```
///
/// Same command as CCM but for the the sequence **sequencename**. Only selected images in the sequence are processed.
///
/// The output sequence name starts with the prefix "ccm" unless otherwise specified with option **-prefix=**
///
/// Links: :ref:`ccm <ccm>`
///
#[derive(Builder)]
pub struct Seqccm {
    #[builder(start_fn, into)]
    sequencename: String,
    prefix: Option<String>
}

impl Command for Seqccm {
    fn name() -> &'static str {
        "seqccm"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.sequencename),
            Argument::option("prefix", self.prefix.as_ref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal() {
        let cmd = Seqccm::builder("sequence").build();
        assert_eq!(cmd.to_args_string(), "seqccm sequence");
    }

    #[test]
    fn with_prefix() {
        let cmd = Seqccm::builder("sequence").prefix("ccm_".to_string()).build();
        assert_eq!(cmd.to_args_string(), "seqccm sequence -prefix=ccm_");
    }
}
