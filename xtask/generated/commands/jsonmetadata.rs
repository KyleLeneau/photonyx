use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// jsonmetadata FITS_file [-stats_from_loaded] [-nostats] [-out=]
/// ```
///
/// Dumps metadata and statistics of the currently loaded image in JSON form. The file name is required, even if the image is already loaded. Image data may not be read from the file if it is the current loaded image and if the **-stats_from_loaded** option is passed. Statistics can be disabled by providing the **-nostats** option. A file containing the JSON data is created with default file name '$(FITS_file_without_ext).json' and can be changed with the **-out=** option
///
#[derive(Builder)]
pub struct Jsonmetadata {}

impl Command for Jsonmetadata {
    fn name() -> &'static str {
        "jsonmetadata"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
