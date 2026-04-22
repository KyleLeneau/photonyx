use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// select sequencename from to
/// ```
///
/// This command allows easy mass selection of images in the sequence **sequencename** (from **from** to **to** included). This is a selection for later processing.
/// See also UNSELECT
///
/// Links: :ref:`unselect <unselect>`
///
/// |
/// Examples:
///
/// `select . 0 0`
/// selects the first of the currently loaded sequence
///
/// `select sequencename 1000 1200`
/// selects 201 images starting from number 1000 in sequence named sequencename
///
/// The second number can be greater than the number of images to just go up to the end.
///
#[derive(Builder)]
pub struct Select {
    #[builder(start_fn, into)]
    sequencename: String,
    #[builder(start_fn)]
    from: u8,
    #[builder(start_fn)]
    to: u8,
}

impl Command for Select {
    fn name() -> &'static str {
        "select"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.sequencename),
            Argument::positional(self.from.to_string()),
            Argument::positional(self.to.to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_range() {
        let cmd = Select::builder("sequence", 0, 10).build();
        assert_eq!(cmd.to_args_string(), "select sequence 0 10");
    }

    #[test]
    fn current_sequence_dot() {
        let cmd = Select::builder(".", 0, 0).build();
        assert_eq!(cmd.to_args_string(), "select . 0 0");
    }

    #[test]
    fn large_range() {
        let cmd = Select::builder("myseq", 100, 200).build();
        assert_eq!(cmd.to_args_string(), "select myseq 100 200");
    }
}
