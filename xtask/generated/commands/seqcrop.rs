use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqcrop sequencename x y width height [-prefix=]
/// ```
///
/// Crops the sequence given in argument **sequencename**. Only selected images in the sequence are processed.
///
/// The crop selection is specified by the upper left corner position **x** and **y** and the selection **width** and **height**, like for CROP.
/// The output sequence name starts with the prefix "cropped\_" unless otherwise specified with **-prefix=** option
///
/// Links: :ref:`crop <crop>`
///
#[derive(Builder)]
pub struct Seqcrop {}

impl Command for Seqcrop {
    fn name() -> &'static str {
        "seqcrop"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
