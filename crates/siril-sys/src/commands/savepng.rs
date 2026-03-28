use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savepng filename
/// ```
///
/// Saves current image into a PNG file: **filename**.png, with 16 bits per channel if the loaded image is 16 or 32 bits, and 8 bits per channel if the loaded image is 8 bits
///
#[derive(Builder)]
pub struct Savepng {
    #[builder(start_fn, into)]
    filename: String,
}

impl Command for Savepng {
    fn name() -> &'static str {
        "savepng"
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
        let cmd = Savepng::builder("output").build();
        assert_eq!(cmd.to_args_string(), "savepng output");
    }

    #[test]
    fn filename_with_spaces_is_quoted() {
        let cmd = Savepng::builder("my output").build();
        assert_eq!(cmd.to_args_string(), "savepng 'my output'");
    }
}
