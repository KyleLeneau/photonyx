use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// select sequencename from to
/// ```
///
/// This command allows easy mass selection of images in the sequence **sequencename** (from **from** to **to** included). This is a selection for later processing.
/// See also UNSELECT
///
/// Links: :ref:`unselect <unselect>`
///
/// |
/// Examples:
///
/// `select . 0 0`
/// selects the first of the currently loaded sequence
///
/// `select sequencename 1000 1200`
/// selects 201 images starting from number 1000 in sequence named sequencename
///
/// The second number can be greater than the number of images to just go up to the end.
///
#[derive(Builder)]
pub struct Select {}

impl Command for Select {
    fn name() -> &'static str {
        "select"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
