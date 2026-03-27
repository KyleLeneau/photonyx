use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// psf [channel]
/// ```
///
/// Performs a PSF (Point Spread Function) on the selected star and display the results. For headless operation, the selection can be given in pixels using BOXSELECT. If provided, the **channel** argument selects the image channel on which the star will be analyzed. It can be omitted for monochrome images or when run from the GUI with one of the channels active in the view
///
/// Links: :ref:`boxselect <boxselect>`
///
#[derive(Builder)]
pub struct Psf {}

impl Command for Psf {
    fn name() -> &'static str {
        "psf"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
