use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// trixel [-p]
/// ```
///
/// For developers.
///
/// Without any argument, lists all the trixels of level 3 visible in the plate-solved image. The stars from each trixel can then be shown with command CONESEARCH using **-trix=** followed by a visible trixel number
///
/// With argument **-p**, prints out all the valid stars from all the 512 level3 trixels to file "trixels.csv"
///
/// Links: :ref:`conesearch <conesearch>`
///
#[derive(Builder)]
pub struct Trixel {}

impl Command for Trixel {
    fn name() -> &'static str {
        "trixel"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
