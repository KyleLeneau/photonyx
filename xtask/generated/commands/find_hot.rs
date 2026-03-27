use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// find_hot filename cold_sigma hot_sigma
/// ```
///
/// Saves a list file **filename** (text format) in the working directory which contains the coordinates of the pixels which have an intensity **hot_sigma** times higher and **cold_sigma** lower than standard deviation, extracted from the loaded image. We generally use this command on a master-dark file. The COSME command can apply this list of bad pixels to a loaded image, see also SEQCOSME to apply it to a sequence
///
/// Links: :ref:`cosme <cosme>`, :ref:`seqcosme <seqcosme>`
///
/// |
/// Lines ``P x y type`` will fix the pixel at coordinates (x, y) type is an optional character (C or H) specifying to Siril if the current pixel is cold or hot. This line is created by the command FIND_HOT but you also can add some lines manually:
/// Lines ``C x 0 type`` will fix the bad column at coordinates x.
/// Lines ``L y 0 type`` will fix the bad line at coordinates y.
///
#[derive(Builder)]
pub struct FindHot {}

impl Command for FindHot {
    fn name() -> &'static str {
        "find_hot"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
