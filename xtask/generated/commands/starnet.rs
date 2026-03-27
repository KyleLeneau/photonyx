use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// starnet [-stretch] [-upscale] [-stride=value] [-nostarmask]
/// ```
///
/// Calls `StarNet <https://www.starnetastro.com/>`__ to remove stars from the loaded image.
///
/// **Prerequisite:** StarNet is an external program, with no affiliation with Siril, and must be installed correctly prior the first use of this command, with the path to its CLI version installation correctly set in Preferences / Miscellaneous.
///
/// The starless image is loaded on completion, and a star mask image is created in the working directory unless the optional parameter **-nostarmask** is provided.
///
/// Optionally, parameters may be passed to the command:
/// - The option **-stretch** is for use with linear images and will apply a pre-stretch before running StarNet and the inverse stretch to the generated starless and starmask images.
/// - To improve star removal on images with very tight stars, the parameter **-upscale** may be provided. This will upsample the image by a factor of 2 prior to StarNet processing and rescale it to the original size afterwards, at the expense of more processing time.
/// - The optional parameter **-stride=value** may be provided, however the author of StarNet *strongly* recommends that the default stride of 256 be used
///
#[derive(Builder)]
pub struct Starnet {}

impl Command for Starnet {
    fn name() -> &'static str {
        "starnet"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
