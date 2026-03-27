use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// autostretch [-linked] [shadowsclip [targetbg]]
/// ```
///
/// Auto-stretches the currently loaded image, with different parameters for each channel (unlinked) unless **-linked** is passed. Arguments are optional, **shadowclip** is the shadows clipping point, measured in sigma units from the main histogram peak (default is -2.8), **targetbg** is the target background value, giving a final brightness to the image, range [0, 1], default is 0.25. The default values are those used in the Auto-stretch rendering from the GUI.
///
/// Do not use the unlinked version after color calibration, it will alter the white balance
///
#[derive(Builder)]
pub struct Autostretch {}

impl Command for Autostretch {
    fn name() -> &'static str {
        "autostretch"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
