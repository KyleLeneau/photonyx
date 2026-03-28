use bon::Builder;
use std::path::PathBuf;

use crate::commands::{Argument, Command};

/// ```text
/// convert basename [-debayer] [-fitseq] [-ser] [-start=index] [-out=]
/// ```
///
/// Converts all images of the current working directory that are in a supported format into Siril's sequence of FITS images (several files) or a FITS sequence (single file) if **-fitseq** is provided or a SER sequence (single file) if **-ser** is provided. The argument **basename** is the base name of the new sequence, numbers and the extension will be put behind it.
/// For FITS images, Siril will try to make a symbolic link; if not possible, files will be copied. The option **-debayer** applies demosaicing to CFA input images; in this case no symbolic link is done.
/// **-start=index** sets the starting index number, useful to continue an existing sequence (not used with -fitseq or **-ser**; make sure you remove or clear the target .seq if it exists in that case).
/// The **-out=** option changes the output directory to the provided argument.
///
/// See also CONVERTRAW and LINK
///
/// Links: :ref:`convertraw <convertraw>`, :ref:`link <link>`
///
#[derive(Builder)]
pub struct Convert {
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

impl Command for Convert {
    fn name() -> &'static str {
        "convert"
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
        let cmd = Convert::builder("light").build();
        assert_eq!(cmd.to_args_string(), "convert light");
    }

    #[test]
    fn with_debayer_flag() {
        let cmd = Convert::builder("light").debayer(true).build();
        assert_eq!(cmd.to_args_string(), "convert light -debayer");
    }

    #[test]
    fn with_fitseq_flag() {
        let cmd = Convert::builder("light").use_fitseq(true).build();
        assert_eq!(cmd.to_args_string(), "convert light -fitseq");
    }

    #[test]
    fn with_ser_flag() {
        let cmd = Convert::builder("light").use_ser(true).build();
        assert_eq!(cmd.to_args_string(), "convert light -ser");
    }

    #[test]
    fn with_start_index() {
        let cmd = Convert::builder("light").start_index(10u8).build();
        assert_eq!(cmd.to_args_string(), "convert light -start=10");
    }

    #[test]
    fn with_output_dir() {
        let cmd = Convert::builder("light")
            .output_dir(PathBuf::from("/tmp/out"))
            .build();
        assert_eq!(cmd.to_args_string(), "convert light -out=/tmp/out");
    }

    #[test]
    fn basename_with_spaces_is_quoted() {
        let cmd = Convert::builder("my lights").build();
        assert_eq!(cmd.to_args_string(), "convert 'my lights'");
    }

    #[test]
    fn all_options() {
        let cmd = Convert::builder("light")
            .debayer(true)
            .start_index(5u8)
            .output_dir(PathBuf::from("/out"))
            .build();
        let s = cmd.to_args_string();
        assert!(s.starts_with("convert light"));
        assert!(s.contains("-debayer"));
        assert!(s.contains("-start=5"));
        assert!(s.contains("-out=/out"));
    }
}
