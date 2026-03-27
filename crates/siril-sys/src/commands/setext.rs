use bon::Builder;

use crate::{
    FitsExt,
    commands::{Argument, Command},
};

/// ```text
/// setext extension
/// ```
///
/// Sets the extension used and recognized by sequences.
///
/// The argument **extension** can be "fit", "fts" or "fits"
///
#[derive(Builder)]
pub struct SetExt {
    #[builder(start_fn)]
    extension: FitsExt,
}

impl Command for SetExt {
    fn name() -> &'static str {
        "setext"
    }

    fn args(&self) -> Vec<super::Argument> {
        vec![Argument::positional(self.extension.to_string())]
    }
}
