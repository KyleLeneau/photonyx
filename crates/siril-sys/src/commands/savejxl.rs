use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// savejxl filename [-effort=] [-quality=] [-8bit]
/// ```
///
/// Saves current image into a JPG XL file: **filename**.jxl.
///
/// All other arguments are optional. The quality setting expresses a maximum permissible distance between the original and the compressed image: the **-quality=** argument may be provided and must be specified as a floating point number between 0.0 and 10.0. A higher quality means better quality, but larger file size. Quality = 10.0 is mathematically lossless, quality = 9.0 is visually lossless and quality = 0 is visually poor but gives very small file sizes. The default value is 9.0; typical values range from 7.0 to 10.0. The compression effort can be adjusted using the optional **-effort=** value, 9 being the most effort but very slow, while a lower value increases the compression ratio. Values above 7 are not recommended as they can be very slow and produce little if any benefit to file size, in fact sometimes effort = 9 can produce larger files. If this argument is omitted the default value of 7 is used. An option **-8bit** may be provided to force output to be 8 bits per pixel
///
#[derive(Builder)]
pub struct Savejxl {
    #[builder(start_fn, into)]
    filename: String,
    effort: Option<i8>,
    quality: Option<f32>,
    #[builder(default = false)]
    eight_bit: bool,
}

impl Command for Savejxl {
    fn name() -> &'static str {
        "savejxl"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.filename.clone()),
            Argument::option("effort", self.effort),
            Argument::option("quality", self.quality),
            Argument::flag_option("8bit", self.eight_bit),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_only() {
        let cmd = Savejxl::builder("output").build();
        assert_eq!(cmd.to_args_string(), "savejxl output");
    }

    #[test]
    fn with_effort() {
        let cmd = Savejxl::builder("output").effort(7).build();
        assert_eq!(cmd.to_args_string(), "savejxl output -effort=7");
    }

    #[test]
    fn with_quality() {
        let cmd = Savejxl::builder("output").quality(9.0).build();
        assert_eq!(cmd.to_args_string(), "savejxl output -quality=9");
    }

    #[test]
    fn eight_bit_flag() {
        let cmd = Savejxl::builder("output").eight_bit(true).build();
        assert_eq!(cmd.to_args_string(), "savejxl output -8bit");
    }

    #[test]
    fn eight_bit_false_omitted() {
        let cmd = Savejxl::builder("output").eight_bit(false).build();
        assert!(!cmd.to_args_string().contains("8bit"));
    }

    #[test]
    fn all_options() {
        let cmd = Savejxl::builder("output")
            .effort(7)
            .quality(9.0)
            .eight_bit(true)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "savejxl output -effort=7 -quality=9 -8bit"
        );
    }

    #[test]
    fn filename_with_spaces_is_quoted() {
        let cmd = Savejxl::builder("my image").build();
        assert_eq!(cmd.to_args_string(), "savejxl 'my image'");
    }
}
