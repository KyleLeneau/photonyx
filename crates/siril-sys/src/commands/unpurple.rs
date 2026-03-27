use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// unpurple [-starmask] [-blue=value] [-thresh=value]
/// ```
///
/// Applies a cosmetic filter to reduce effects of purple fringing on stars.
///
/// If the **-starmask** parameter is given, a star mask will be used to identify areas of the image to affect. If a Dynamic PSF has already been run, this will be used for the starmask, otherwise one will be created automatically. The **-mod=** parameter should be given a value somewhere around 0.14 to reduce the amount of purple. The **-thresh=** will specify the size modifier for each star in the starmask and should be large enough to cause the stars to be entirely processed without remaining purple fringing. The value should between 0 and 1, typically around 0.5.
/// If the **-starmask** parameter is not given, the purple reduction will be applied across the entire image for any purple pixels with a luminance value higher than the given **-thresh=**. In this case, the **-thresh=** value should be reasonably low. This mode is useful for starmasks or other images without nebula or galaxy
///
/// Links: :ref:`psf <psf>`
///
#[derive(Builder)]
pub struct Unpurple {}

impl Command for Unpurple {
    fn name() -> &'static str {
        "unpurple"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
