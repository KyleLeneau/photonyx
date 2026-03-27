use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// cosme [filename].lst
/// ```
///
/// Applies the local mean to a set of pixels on the loaded image (cosmetic correction). The coordinates of these pixels are in a text file [.lst file], the FIND_HOT command can also create it for single hot pixels, but manual operation is needed to remove rows or columns. COSME is adapted to correct residual hot and cold pixels after calibration.
/// Instead of providing the list of bad pixels, it's also possible to detect them in the current image using the FIND_COSME command
///
/// Links: :ref:`find_hot <find_hot>`, :ref:`find_cosme <find_cosme>`
///
/// File format for the bad pixels list:
/// * Lines in the form `P x y` will fix the pixel at coordinates (x, y) type is an optional character (C or H) specifying to Siril if the current pixel is cold or hot. This line is created by the command FIND_HOT but you also can add the two following line types manually
/// * Lines in the form `C x 0` will fix the bad column at coordinates x.
/// * Lines in the form `L y 0` will fix the bad line at coordinates y.
///
#[derive(Builder)]
pub struct Cosme {}

impl Command for Cosme {
    fn name() -> &'static str {
        "cosme"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
