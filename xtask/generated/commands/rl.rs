use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// rl [-loadpsf=] [-alpha=] [-iters=] [-stop=] [-gdstep=] [-tv] [-fh] [-mul]
/// ```
///
/// Restores an image using the Richardson-Lucy method.
///
/// Optionally, a PSF may be loaded using the argument **-loadpsf=\ filename** (created with MAKEPSF).
///
/// The number of iterations is provide by **-iters** (the default is 10).
///
/// The type of regularization can be set with **-tv** for Total Variation, or **-fh** for the Frobenius norm of the Hessian matrix (the default is none) and **-alpha=** provides the regularization strength (lower value = more regularization, default = 3000).
///
/// By default the gradient descent method is used with a default step size of 0.0005, however the multiplicative method may be specified with **-mul**.
///
/// The stopping criterion may be activated by specifying a stopping limit with **-stop=**
///
/// Links: :ref:`psf <psf>`, :ref:`makepsf <makepsf>`
///
#[derive(Builder)]
pub struct Rl {}

impl Command for Rl {
    fn name() -> &'static str {
        "rl"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
