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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_extension() {
        let cmd = SetExt::builder(FitsExt::FIT).build();
        assert_eq!(cmd.to_args_string(), "setext fit");
    }

    #[test]
    fn fits_extension() {
        let cmd = SetExt::builder(FitsExt::FITS).build();
        assert_eq!(cmd.to_args_string(), "setext fits");
    }

    #[test]
    fn fts_extension() {
        let cmd = SetExt::builder(FitsExt::FTS).build();
        assert_eq!(cmd.to_args_string(), "setext fts");
    }
}
