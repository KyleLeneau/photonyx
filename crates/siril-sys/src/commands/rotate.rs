use bon::Builder;

use crate::{
    PixelInterpolation,
    commands::{Argument, Command},
};

/// ```text
/// rotate degree [-nocrop] [-interp=] [-noclamp]
/// ```
///
/// Rotates the loaded image by an angle of **degree** value. The option **-nocrop** can be added to avoid cropping to the image size (black borders will be added).
///
/// Note: if a selection is active, i.e. by using a command \`boxselect\` before \`rotate\`, the resulting image will be a rotated crop. In this particular case, the option **-nocrop** will be ignored if passed.
///
/// The pixel interpolation method can be specified with the **-interp=** argument followed by one of the methods in the list **no**\ [ne], **ne**\ [arest], **cu**\ [bic], **la**\ [nczos4], **li**\ [near], **ar**\ [ea]}. If **none** is passed, the transformation is forced to shift and a pixel-wise shift is applied to each image without any interpolation.
/// Clamping of the bicubic and lanczos4 interpolation methods is the default, to avoid artefacts, but can be disabled with the **-noclamp** argument
///
#[derive(Builder)]
pub struct Rotate {
    #[builder(start_fn)]
    degree: f32,
    #[builder(default = false)]
    no_crop: bool,
    interp: Option<PixelInterpolation>,
    #[builder(default = false)]
    no_clamp: bool,
}

impl Command for Rotate {
    fn name() -> &'static str {
        "rotate"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.degree.to_string()),
            Argument::flag_option("nocrop", self.no_crop),
            Argument::option("interp", self.interp.as_ref()),
            Argument::flag_option("noclamp", self.no_clamp),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PixelInterpolation;

    #[test]
    fn minimal() {
        let cmd = Rotate::builder(90.0).build();
        assert_eq!(cmd.to_args_string(), "rotate 90");
    }

    #[test]
    fn no_crop() {
        let cmd = Rotate::builder(45.0).no_crop(true).build();
        assert_eq!(cmd.to_args_string(), "rotate 45 -nocrop");
    }

    #[test]
    fn with_interp() {
        let cmd = Rotate::builder(90.0)
            .interp(PixelInterpolation::Cubic)
            .build();
        assert_eq!(cmd.to_args_string(), "rotate 90 -interp=cubic");
    }

    #[test]
    fn no_clamp() {
        let cmd = Rotate::builder(90.0).no_clamp(true).build();
        assert_eq!(cmd.to_args_string(), "rotate 90 -noclamp");
    }

    #[test]
    fn all_options() {
        let cmd = Rotate::builder(30.0)
            .no_crop(true)
            .interp(PixelInterpolation::Lanczos4)
            .no_clamp(true)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "rotate 30 -nocrop -interp=lanczos4 -noclamp"
        );
    }
}
