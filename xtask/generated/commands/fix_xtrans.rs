use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// fix_xtrans
/// ```
///
/// Fixes the Fujifilm X-Trans Auto Focus pixels in the loaded image.
///
/// Indeed, because of the phase detection auto focus system, the photosites used for auto focus get a little less light than the surrounding photosites. The camera compensates for this and increases the values from these specific photosites giving a visible square in the middle of the dark/bias frames
///
#[derive(Builder)]
pub struct FixXtrans {}

impl Command for FixXtrans {
    fn name() -> &'static str {
        "fix_xtrans"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
