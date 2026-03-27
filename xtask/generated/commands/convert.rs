use bon::Builder;

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
pub struct Convert {}

impl Command for Convert {
    fn name() -> &'static str {
        "convert"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
