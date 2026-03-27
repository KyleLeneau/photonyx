use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// wiener [-loadpsf=] [-alpha=]
/// ```
///
/// Restores an image using the Wiener deconvolution method.
///
/// Optionally, a PSF created by MAKEPSF may be loaded using the argument **-loadpsf=\ filename**.
///
/// The parameter **-alpha=** provides the Gaussian noise modelled regularization factor
///
/// Links: :ref:`psf <psf>`, :ref:`makepsf <makepsf>`
///
#[derive(Builder)]
pub struct Wiener {}

impl Command for Wiener {
    fn name() -> &'static str {
        "wiener"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
