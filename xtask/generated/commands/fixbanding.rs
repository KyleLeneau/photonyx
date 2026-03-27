use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// fixbanding amount sigma [-vertical]
/// ```
///
/// Tries to remove the horizontal or vertical banding in the loaded image.
/// **amount** defines the amount of correction, between 0 and 4.
/// **sigma** defines the highlight protection level of the algorithm, higher sigma gives higher protection, between 0 and 5. Values of 1 and 1 are often good enough.
/// **-vertical** option enables to perform vertical banding removal, horizontal is the default
///
#[derive(Builder)]
pub struct Fixbanding {}

impl Command for Fixbanding {
    fn name() -> &'static str {
        "fixbanding"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
