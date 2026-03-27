use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqheader sequencename keyword [keyword2 ...] [-sel] [-out=file.csv]
/// ```
///
/// Prints the FITS header value corresponding to the given keys for all images in the sequence. You can write several keys in a row, separated by a space. The **-out=** option, followed by a file name, allows you to print the output in a csv file. The **-sel** option limits the output to the images selected in the sequence
///
#[derive(Builder)]
pub struct Seqheader {}

impl Command for Seqheader {
    fn name() -> &'static str {
        "seqheader"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
