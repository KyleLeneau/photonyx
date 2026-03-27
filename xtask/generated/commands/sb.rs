use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// sb [-loadpsf=] [-alpha=] [-iters=]
/// ```
///
/// Restores an image using the Split Bregman method.
///
/// Optionally, a PSF may be loaded using the argument **-loadpsf=\ filename**.
///
/// The number of iterations is provide by **-iters** (the default is 1).
///
/// The regularization factor **-alpha=** provides the regularization strength (lower value = more regularization, default = 3000)
///
/// Links: :ref:`psf <psf>`
///
#[derive(Builder)]
pub struct Sb {}

impl Command for Sb {
    fn name() -> &'static str {
        "sb"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
