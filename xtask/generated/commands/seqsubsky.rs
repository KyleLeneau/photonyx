use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqsubsky sequencename { -rbf | degree } [-nodither] [-samples=20] [-tolerance=1.0] [-smooth=0.5] [-prefix=]
/// ```
///
/// Same command as SUBSKY but for the sequence **sequencename**.
/// Dithering, required for low dynamic gradients, can be disabled with **-nodither**. Note that the **-existing** option is not available for sequence background removal, as the frames of a sequence are not necessarily always aligned.
///
/// The output sequence name starts with the prefix "bkg\_" unless otherwise specified with **-prefix=** option. Only selected images in the sequence are processed
///
/// Links: :ref:`subsky <subsky>`
///
#[derive(Builder)]
pub struct Seqsubsky {}

impl Command for Seqsubsky {
    fn name() -> &'static str {
        "seqsubsky"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
