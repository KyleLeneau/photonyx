use std::path::PathBuf;

use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// convertraw basename [-debayer] [-fitseq] [-ser] [-start=index] [-out=]
/// ```
///
/// Same as CONVERT but converts only DSLR RAW files found in the current working directory
///
/// Links: :ref:`convert <convert>`
///
#[derive(Builder)]
pub struct Convertraw {
    #[builder(start_fn, into)]
    base_name: String,
    #[builder(default = false)]
    debayer: bool,
    #[builder(default = false)]
    use_fitseq: bool,
    #[builder(default = false)]
    use_ser: bool,
    start_index: Option<u8>,
    output_dir: Option<PathBuf>,
}

impl Command for Convertraw {
    fn name() -> &'static str {
        "convertraw"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.base_name),
            Argument::flag_option("debayer", self.debayer),
            Argument::flag_option("fitseq", self.use_fitseq),
            Argument::flag_option("ser", self.use_ser),
            Argument::option("start", self.start_index),
            Argument::option("out", self.output_dir.as_ref().map(|v| v.display())),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_basename_only() {
        let cmd = Convertraw::builder("raw").build();
        assert_eq!(cmd.to_args_string(), "convertraw raw");
    }

    #[test]
    fn with_debayer_flag() {
        let cmd = Convertraw::builder("raw").debayer(true).build();
        assert_eq!(cmd.to_args_string(), "convertraw raw -debayer");
    }

    #[test]
    fn with_fitseq_flag() {
        let cmd = Convertraw::builder("raw").use_fitseq(true).build();
        assert_eq!(cmd.to_args_string(), "convertraw raw -fitseq");
    }

    #[test]
    fn with_ser_flag() {
        let cmd = Convertraw::builder("raw").use_ser(true).build();
        assert_eq!(cmd.to_args_string(), "convertraw raw -ser");
    }

    #[test]
    fn with_start_index() {
        let cmd = Convertraw::builder("raw").start_index(5u8).build();
        assert_eq!(cmd.to_args_string(), "convertraw raw -start=5");
    }

    #[test]
    fn with_output_dir() {
        let cmd = Convertraw::builder("raw")
            .output_dir(PathBuf::from("/tmp/out"))
            .build();
        assert_eq!(cmd.to_args_string(), "convertraw raw -out=/tmp/out");
    }

    #[test]
    fn all_options() {
        let cmd = Convertraw::builder("raw")
            .debayer(true)
            .start_index(1u8)
            .output_dir(PathBuf::from("/out"))
            .build();
        let s = cmd.to_args_string();
        assert!(s.starts_with("convertraw raw"));
        assert!(s.contains("-debayer"));
        assert!(s.contains("-start=1"));
        assert!(s.contains("-out=/out"));
    }
}
