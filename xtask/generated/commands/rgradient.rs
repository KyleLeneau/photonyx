use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// rgradient xc yc dR dalpha
/// ```
///
/// Creates two images, with a radial shift (**dR** in pixels) and a rotational shift (**dalpha** in degrees) with respect to the point (**xc**, **yc**).
///
/// Between these two images, the shifts have the same amplitude, but an opposite sign. The two images are then added to create the final image. This process is also called Larson Sekanina filter
///
#[derive(Builder)]
pub struct Rgradient {}

impl Command for Rgradient {
    fn name() -> &'static str {
        "rgradient"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
