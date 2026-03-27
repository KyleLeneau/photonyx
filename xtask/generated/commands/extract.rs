use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// extract NbPlans
/// ```
///
/// Extracts **NbPlans** planes of wavelet domain of the loaded image.
/// See also WAVELET and WRECONS. For color extraction, see SPLIT
///
/// Links: :ref:`wavelet <wavelet>`, :ref:`wrecons <wrecons>`, :ref:`split <split>`
///
#[derive(Builder)]
pub struct Extract {}

impl Command for Extract {
    fn name() -> &'static str {
        "extract"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
