use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// unselect sequencename from to
/// ```
///
/// Allows easy mass unselection of images in the sequence **sequencename** (from **from** to **to** included). See SELECT
///
/// Links: :ref:`select <select>`
///
#[derive(Builder)]
pub struct Unselect {
    #[builder(start_fn, into)]
    sequencename: String,
    #[builder(start_fn)]
    from: u8,
    #[builder(start_fn)]
    to: u8,
}

impl Command for Unselect {
    fn name() -> &'static str {
        "unselect"
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
        let cmd = Unselect::builder("sequence", 0, 10).build();
        assert_eq!(cmd.to_args_string(), "unselect sequence 0 10");
    }

    #[test]
    fn current_sequence_dot() {
        let cmd = Unselect::builder(".", 0, 0).build();
        assert_eq!(cmd.to_args_string(), "unselect . 0 0");
    }

    #[test]
    fn large_range() {
        let cmd = Unselect::builder("myseq", 50, 255).build();
        assert_eq!(cmd.to_args_string(), "unselect myseq 50 255");
    }
}
