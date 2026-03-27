use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// resample { factor | -width= | -height= | -maxdim= } [-interp=] [-noclamp]
/// ```
///
/// Resamples the loaded image, either with a factor **factor** or for the target width or height provided by either of **-width=**, **-height=** or **-maxdim=**. This is generally used to resize images: a factor of 0.5 divides size by 2. The **-maxdim** argument can be used to resize the longest dimension of the image to a set size, which can be useful for optimizing images for certain websites, e.g. social media websites.
/// In the graphical user interface, we can see that several interpolation algorithms are proposed.
///
/// The pixel interpolation method can be specified with the **-interp=** argument followed by one of the methods in the list **no**\ [ne], **ne**\ [arest], **cu**\ [bic], **la**\ [nczos4], **li**\ [near], **ar**\ [ea]}.
/// Clamping of the bicubic and lanczos4 interpolation methods is the default, to avoid artefacts, but can be disabled with the **-noclamp** argument
///
#[derive(Builder)]
pub struct Resample {}

impl Command for Resample {
    fn name() -> &'static str {
        "resample"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
