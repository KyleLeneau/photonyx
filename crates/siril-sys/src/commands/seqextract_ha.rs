use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqextract_Ha sequencename [-prefix=] [-upscale]
/// ```
///
/// Same command as EXTRACT_HA but for the sequence **sequencename**.
///
/// The output sequence name starts with the prefix "Ha\_" unless otherwise specified with option **-prefix=**
///
#[derive(Builder)]
pub struct SeqextractHa {
    #[builder(start_fn, into)]
    sequence: String,
    #[builder(into)]
    prefix: Option<String>,
    #[builder(default = false)]
    upscale: bool,
}

impl Command for SeqextractHa {
    fn name() -> &'static str {
        "seqextract_Ha"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.sequence),
            Argument::option("prefix", self.prefix.as_deref()),
            Argument::flag_option("upscale", self.upscale),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_only() {
        let cmd = SeqextractHa::builder("lights").build();
        assert_eq!(cmd.to_args_string(), "seqextract_Ha lights");
    }

    #[test]
    fn custom_prefix() {
        let cmd = SeqextractHa::builder("lights").prefix("Ha2_").build();
        assert_eq!(cmd.to_args_string(), "seqextract_Ha lights -prefix=Ha2_");
    }

    #[test]
    fn upscale() {
        let cmd = SeqextractHa::builder("lights").upscale(true).build();
        assert_eq!(cmd.to_args_string(), "seqextract_Ha lights -upscale");
    }

    #[test]
    fn prefix_and_upscale() {
        let cmd = SeqextractHa::builder("lights")
            .prefix("Ha2_")
            .upscale(true)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "seqextract_Ha lights -prefix=Ha2_ -upscale"
        );
    }
}
