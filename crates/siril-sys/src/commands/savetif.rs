use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savetif filename [-astro] [-deflate]
/// ```
///
/// Saves current image under the form of a uncompressed TIFF file with 16-bit per channel: **filename**.tif. The option **-astro** allows saving in Astro-TIFF format, while **-deflate** enables compression.
///
/// See also SAVETIF32 and SAVETIF8
///
#[derive(Builder)]
pub struct Savetif {
    #[builder(start_fn, into)]
    filename: String,
    #[builder(default = false)]
    astro_tiff: bool,
    #[builder(default = false)]
    deflate: bool,
}

impl Command for Savetif {
    fn name() -> &'static str {
        "savetif"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.filename.clone()),
            Argument::flag_option("astro", self.astro_tiff),
            Argument::flag_option("deflate", self.deflate),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_only() {
        let cmd = Savetif::builder("output").build();
        assert_eq!(cmd.to_args_string(), "savetif output");
    }

    #[test]
    fn astro_tiff_flag() {
        let cmd = Savetif::builder("output").astro_tiff(true).build();
        assert_eq!(cmd.to_args_string(), "savetif output -astro");
    }

    #[test]
    fn deflate_flag() {
        let cmd = Savetif::builder("output").deflate(true).build();
        assert_eq!(cmd.to_args_string(), "savetif output -deflate");
    }

    #[test]
    fn both_flags() {
        let cmd = Savetif::builder("output")
            .astro_tiff(true)
            .deflate(true)
            .build();
        assert_eq!(cmd.to_args_string(), "savetif output -astro -deflate");
    }

    #[test]
    fn false_flags_omitted() {
        let cmd = Savetif::builder("output")
            .astro_tiff(false)
            .deflate(false)
            .build();
        assert_eq!(cmd.to_args_string(), "savetif output");
    }

    #[test]
    fn filename_with_spaces_is_quoted() {
        let cmd = Savetif::builder("my image").build();
        assert_eq!(cmd.to_args_string(), "savetif 'my image'");
    }
}
