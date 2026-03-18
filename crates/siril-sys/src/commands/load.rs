use bon::Builder;

use crate::commands::{Argument, Command};

/// .. code-block:: text
///
///        load filename[.ext]
///
/// Loads the image **filename** from the current working directory, which becomes the 'currently loaded image' used in many of the single-image commands.
/// It first attempts to load **filename**, then **filename**.fit, **filename**.fits and finally all supported formats.
/// This scheme is applicable to every Siril command that involves reading files
#[derive(Builder)]
pub struct Load {
    #[builder(start_fn)]
    filename: String,
}

impl Command for Load {
    fn name() -> &'static str {
        "load"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(self.filename.clone())]
    }
}
