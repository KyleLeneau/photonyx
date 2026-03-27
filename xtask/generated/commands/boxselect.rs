use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// boxselect [-clear] [x y width height]
/// ```
///
/// Make a selection area in the currently loaded image with the arguments **x**, **y**, **width** and **height**, with **x** and **y** being the coordinates of the top left corner starting at (0, 0), and **width** and **height**, the size of the selection. The **-clear** argument deletes any selection area. If no argument is passed, the current selection is printed
///
#[derive(Builder)]
pub struct Boxselect {}

impl Command for Boxselect {
    fn name() -> &'static str {
        "boxselect"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
