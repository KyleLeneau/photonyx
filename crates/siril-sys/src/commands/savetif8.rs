use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savetif8 filename [-astro] [-deflate]
/// ```
///
/// Same command as SAVETIF but the output file is saved in 8-bit per channel: **filename**.tif. The option **-astro** allows saving in Astro-TIFF format, while **-deflate** enables compression
///
/// Links: :ref:`savetif <savetif>`
///
#[derive(Builder)]
pub struct Savetif8 {
    #[builder(start_fn, into)]
    filename: String,
    #[builder(default = false)]
    astro_tiff: bool,
    #[builder(default = false)]
    deflate: bool,
}

impl Command for Savetif8 {
    fn name() -> &'static str {
        "savetif8"
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
        let cmd = Savetif8::builder("output").build();
        assert_eq!(cmd.to_args_string(), "savetif8 output");
    }

    #[test]
    fn astro_tiff_flag() {
        let cmd = Savetif8::builder("output").astro_tiff(true).build();
        assert_eq!(cmd.to_args_string(), "savetif8 output -astro");
    }

    #[test]
    fn deflate_flag() {
        let cmd = Savetif8::builder("output").deflate(true).build();
        assert_eq!(cmd.to_args_string(), "savetif8 output -deflate");
    }

    #[test]
    fn both_flags() {
        let cmd = Savetif8::builder("output")
            .astro_tiff(true)
            .deflate(true)
            .build();
        assert_eq!(cmd.to_args_string(), "savetif8 output -astro -deflate");
    }

    #[test]
    fn false_flags_omitted() {
        let cmd = Savetif8::builder("output")
            .astro_tiff(false)
            .deflate(false)
            .build();
        assert_eq!(cmd.to_args_string(), "savetif8 output");
    }

    #[test]
    fn filename_with_spaces_is_quoted() {
        let cmd = Savetif8::builder("my image").build();
        assert_eq!(cmd.to_args_string(), "savetif8 'my image'");
    }
}
