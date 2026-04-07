use std::ops::Deref;

use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savejpg filename [quality]
/// ```
///
/// Saves current image into a JPG file: **filename**.jpg.
///
/// The compression quality can be adjusted using the optional **quality** value, 100 being the best and default, while a lower value increases the compression ratio
///
#[derive(Builder)]
pub struct Savejpg {
    #[builder(start_fn, into)]
    filename: String,
    quality: Option<i8>,
}

impl Command for Savejpg {
    fn name() -> &'static str {
        "savejpg"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.filename.deref()),
            Argument::positional_option(self.quality),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_only() {
        let cmd = Savejpg::builder("output").build();
        assert_eq!(cmd.to_args_string(), "savejpg output");
    }

    #[test]
    fn filename_with_quality() {
        let cmd = Savejpg::builder("output").quality(80).build();
        assert_eq!(cmd.to_args_string(), "savejpg output 80");
    }

    #[test]
    fn quality_100_best() {
        let cmd = Savejpg::builder("output").quality(100).build();
        assert_eq!(cmd.to_args_string(), "savejpg output 100");
    }

    #[test]
    fn filename_with_spaces_is_quoted() {
        let cmd = Savejpg::builder("my image").build();
        assert_eq!(cmd.to_args_string(), "savejpg 'my image'");
    }
}
