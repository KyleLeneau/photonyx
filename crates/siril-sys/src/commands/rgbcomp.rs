use bon::Builder;

use crate::{RGBImage, commands::{Argument, Command}};

/// ```text
/// rgbcomp red green blue [-out=result_filename] [-nosum]
/// rgbcomp -lum=image { rgb_image | red green blue } [-out=result_filename] [-nosum]
/// ```
///
/// Creates an RGB composition using three independent images, or an LRGB composition using the optional luminance image and three monochrome images or a color image. Result image is called composed_rgb.fit or composed_lrgb.fit unless another name is provided in the optional argument. Another optional argument, **-nosum** tells Siril not to sum exposure times. This impacts FITS keywords such as LIVETIME and STACKCNT
///
#[derive(Builder)]
pub struct Rgbcomp {
    #[builder(start_fn)]
    rgb_image: RGBImage,
    #[builder(into)]
    luminance: Option<String>,
    #[builder(into)]
    out: Option<String>,
    #[builder(default = false)]
    no_sum: bool,
}

impl Command for Rgbcomp {
    fn name() -> &'static str {
        "rgbcomp"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![Argument::option("lum", self.luminance.clone())];

        match &self.rgb_image {
            RGBImage::Single(rgb) => {
                args.push(Argument::positional(rgb));
            },
            RGBImage::RGB(red, green, blue) => {
                args.push(Argument::positional(red));
                args.push(Argument::positional(green));
                args.push(Argument::positional(blue));
            },
        }

        args.push(Argument::option("out", self.out.clone()));
        args.push(Argument::flag_option("nosum", self.no_sum));
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_rgb_image() {
        let cmd = Rgbcomp::builder(RGBImage::Single("rgb.fit".into())).build();
        assert_eq!(cmd.to_args_string(), "rgbcomp rgb.fit");
    }

    #[test]
    fn three_channel_rgb() {
        let cmd = Rgbcomp::builder(RGBImage::RGB(
            "red.fit".into(),
            "green.fit".into(),
            "blue.fit".into(),
        ))
        .build();
        assert_eq!(cmd.to_args_string(), "rgbcomp red.fit green.fit blue.fit");
    }

    #[test]
    fn lrgb_with_single_rgb() {
        let cmd = Rgbcomp::builder(RGBImage::Single("rgb.fit".into()))
            .luminance("lum.fit")
            .build();
        assert_eq!(cmd.to_args_string(), "rgbcomp -lum=lum.fit rgb.fit");
    }

    #[test]
    fn lrgb_with_three_channels() {
        let cmd = Rgbcomp::builder(RGBImage::RGB(
            "red.fit".into(),
            "green.fit".into(),
            "blue.fit".into(),
        ))
        .luminance("lum.fit")
        .build();
        assert_eq!(
            cmd.to_args_string(),
            "rgbcomp -lum=lum.fit red.fit green.fit blue.fit"
        );
    }

    #[test]
    fn out_option() {
        let cmd = Rgbcomp::builder(RGBImage::Single("rgb.fit".into()))
            .out("result.fit")
            .build();
        assert_eq!(cmd.to_args_string(), "rgbcomp rgb.fit -out=result.fit");
    }

    #[test]
    fn nosum_flag() {
        let cmd = Rgbcomp::builder(RGBImage::Single("rgb.fit".into()))
            .no_sum(true)
            .build();
        assert_eq!(cmd.to_args_string(), "rgbcomp rgb.fit -nosum");
    }

    #[test]
    fn nosum_false_omitted() {
        let cmd = Rgbcomp::builder(RGBImage::Single("rgb.fit".into()))
            .no_sum(false)
            .build();
        assert!(!cmd.to_args_string().contains("nosum"));
    }

    #[test]
    fn full_lrgb_invocation() {
        let cmd = Rgbcomp::builder(RGBImage::RGB(
            "red.fit".into(),
            "green.fit".into(),
            "blue.fit".into(),
        ))
        .luminance("lum.fit")
        .out("composed_lrgb.fit")
        .no_sum(true)
        .build();
        assert_eq!(
            cmd.to_args_string(),
            "rgbcomp -lum=lum.fit red.fit green.fit blue.fit -out=composed_lrgb.fit -nosum"
        );
    }
}
