use bon::Builder;

use crate::commands::{Argument, Command};

/// .. code-block:: text
///
///     save filename [-chksum]
///
/// Saves current image to **filename**.fit (or .fits, depending on your preferences, see SETEXT)
/// in the current working directory. The image remains loaded. **filename** can contain a path
/// as long as the directory already exists. The **-chksum** option stores checksum keywords
/// (CHECKSUM and DATASUM) in the FITS header
///
/// Links: :ref:`setext <setext>`
#[derive(Builder)]
pub struct Save {
    #[builder(start_fn)]
    filename: String,
    #[builder(default = false)]
    chksum: bool,
}

impl Command for Save {
    fn name() -> &'static str {
        "save"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.filename),
            Argument::flag("chksum", self.chksum),
        ]
    }
}
