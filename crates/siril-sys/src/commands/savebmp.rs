use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savebmp filename
/// ```
///
/// Saves current image under the form of a bitmap file with 8-bit per channel: **filename**.bmp (BMP 24-bit)
///
#[derive(Builder)]
pub struct Savebmp {
    #[builder(start_fn, into)]
    filename: String,
}

impl Command for Savebmp {
    fn name() -> &'static str {
        "savebmp"
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
        let cmd = Savebmp::builder("output").build();
        assert_eq!(cmd.to_args_string(), "savebmp output");
    }

    #[test]
    fn filename_with_spaces_is_quoted() {
        let cmd = Savebmp::builder("my output").build();
        assert_eq!(cmd.to_args_string(), "savebmp 'my output'");
    }
}
