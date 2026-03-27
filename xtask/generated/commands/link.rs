use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// link basename [-date] [-start=index] [-out=]
/// ```
///
/// Same as CONVERT but converts only FITS files found in the current working directory. This is useful to avoid conversions of JPEG results or other files that may end up in the directory. The additional argument **-date** enables sorting files with their DATE-OBS value instead of with their name alphanumerically
///
/// Links: :ref:`convert <convert>`
///
#[derive(Builder)]
pub struct Link {}

impl Command for Link {
    fn name() -> &'static str {
        "link"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
