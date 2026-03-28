use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// mirrorx_single image
/// ```
///
/// Flips the image about the horizontal axis, only if needed (if it's not already bottom-up). It takes the image file name as argument, allowing it to avoid reading image data entirely if no flip is required. Image is overwritten if a flip is made
///
#[derive(Builder)]
pub struct MirrorxSingle {
    #[builder(start_fn, into)]
    image: String,
}

impl Command for MirrorxSingle {
    fn name() -> &'static str {
        "mirrorx_single"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(self.image.to_string())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_image_path() {
        let cmd = MirrorxSingle::builder("light_001.fit").build();
        assert_eq!(cmd.to_args_string(), "mirrorx_single light_001.fit");
    }

    #[test]
    fn image_path_with_spaces_is_quoted() {
        let cmd = MirrorxSingle::builder("my image.fit").build();
        assert_eq!(cmd.to_args_string(), "mirrorx_single 'my image.fit'");
    }
}
