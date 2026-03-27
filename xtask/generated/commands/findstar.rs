use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// findstar [-out=] [-layer=] [-maxstars=]
/// ```
///
/// Detects stars in the currently loaded image, having a level greater than a threshold computed by Siril.
/// After that, a PSF is applied and Siril rejects all detected structures that don't fulfill a set of prescribed detection criteria, that can be tuned with command SETFINDSTAR.
/// Finally, an ellipse is drawn around detected stars.
///
/// Optional parameter **-out=** allows the results to be saved to the given path.
/// Option **-layer=** specifies the layer onto which the detection is performed (for color images only).
/// You can also limit the maximum number of stars detected by passing a value to option **-maxstars=**.
///
///
/// See also CLEARSTAR
///
/// Links: :ref:`psf <psf>`, :ref:`setfindstar <setfindstar>`, :ref:`clearstar <clearstar>`
///
#[derive(Builder)]
pub struct Findstar {}

impl Command for Findstar {
    fn name() -> &'static str {
        "findstar"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
