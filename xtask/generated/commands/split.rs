use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// split file1 file2 file3 [-hsl | -hsv | -lab]
/// ```
///
/// Splits the loaded color image into three distinct files (one for each color) and saves them in **file1**.fit, **file2**.fit and **file3**.fit files. A last argument can optionally be supplied, **-hsl**, **-hsv** or **lab** to perform an HSL, HSV or CieLAB extraction. If no option are provided, the extraction is of RGB type, meaning no conversion is done
///
#[derive(Builder)]
pub struct Split {}

impl Command for Split {
    fn name() -> &'static str {
        "split"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
