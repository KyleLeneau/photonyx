use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// binxy coefficient [-sum]
/// ```
///
/// Computes the numerical binning of the in-memory image (sum of the pixels 2x2, 3x3..., like the analogic binning of CCD camera). If the optional argument **-sum** is passed, then the sum of pixels is computed, while it is the average when no optional argument is provided
///
#[derive(Builder)]
pub struct Binxy {}

impl Command for Binxy {
    fn name() -> &'static str {
        "binxy"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
