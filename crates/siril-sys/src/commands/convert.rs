use bon::Builder;
use std::path::PathBuf;

use crate::commands::{Argument, Command};

/// ```text
/// convert basename [-debayer] [-fitseq] [-ser] [-start=index] [-out=]
/// ```
///
/// Converts all images of the current working directory that are in a supported format into
/// Siril's sequence of FITS images (several files) or a FITS sequence (single file) if
/// **-fitseq** is provided or a SER sequence (single file) if **-ser** is provided. The argument
/// **basename** is the base name of the new sequence, numbers and the extension will be put
/// behind it.
///
/// For FITS images, Siril will try to make a symbolic link; if not possible, files will be
/// copied. The option **-debayer** applies demosaicing to CFA input images; in this case no
/// symbolic link is done.
///
/// **-start=index** sets the starting index number, useful to continue an existing sequence
/// (not used with -fitseq or **-ser**; make sure you remove or clear the target .seq if it
/// exists in that case).
///
/// The **-out=** option changes the output directory to the provided argument.
///
/// See also CONVERTRAW and LINK
#[derive(Builder)]
pub struct Convert {
    #[builder(start_fn)]
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
            Argument::flag("debayer", self.debayer),
            Argument::flag("fitseq", self.use_fitseq),
            Argument::flag("ser", self.use_ser),
            Argument::option("start", self.start_index),
            Argument::option("out", self.output_dir.as_ref().map(|v| v.display())),
        ]
    }
}
