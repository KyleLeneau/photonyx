use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// crop [x y width height]
/// ```
///
/// Crops to a selected area of the loaded image.
///
/// If a selection is active, no further arguments are required. Otherwise, or in scripts, arguments have to be given, with **x** and **y** being the coordinates of the top left corner, and **width** and **height** the size of the selection. Alternatively, the selection can be made using the BOXSELECT command
///
/// Links: :ref:`boxselect <boxselect>`
///
#[derive(Builder)]
pub struct Crop {}

impl Command for Crop {
    fn name() -> &'static str {
        "crop"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
