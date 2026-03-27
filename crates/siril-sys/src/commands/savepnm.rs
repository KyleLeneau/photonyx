use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savepnm filename
/// ```
///
/// Saves current image under the form of a NetPBM file format with 16-bit per channel.
///
/// The extension of the output will be **filename**.ppm for RGB image and **filename**.pgm for gray-level image
///
#[derive(Builder)]
pub struct Savepnm {
    #[builder(start_fn)]
    filename: String
}

impl Command for Savepnm {
    fn name() -> &'static str {
        "savepnm"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(self.filename.to_string())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_filename() {
        let cmd = Savepnm::builder("output".to_string()).build();
        assert_eq!(cmd.to_args_string(), "savepnm output");
    }

    #[test]
    fn filename_with_spaces_is_quoted() {
        let cmd = Savepnm::builder("my output".to_string()).build();
        assert_eq!(cmd.to_args_string(), "savepnm 'my output'");
    }
}
