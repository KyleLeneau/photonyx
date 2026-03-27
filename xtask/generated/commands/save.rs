use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// save filename [-chksum]
/// ```
///
/// Saves current image to **filename**.fit (or .fits, depending on your preferences, see SETEXT) in the current working directory. The image remains loaded. **filename** can contain a path as long as the directory already exists. The **-chksum** option stores checksum keywords (CHECKSUM and DATASUM) in the FITS header
///
/// Links: :ref:`setext <setext>`
///
#[derive(Builder)]
pub struct Save {}

impl Command for Save {
    fn name() -> &'static str {
        "save"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
