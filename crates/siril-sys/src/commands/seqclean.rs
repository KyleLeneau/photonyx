use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqclean sequencename [-reg] [-stat] [-sel]
/// ```
///
/// This command clears selection, registration and/or statistics data stored for the sequence **sequencename**.
///
/// You can specify to clear only registration, statistics and/or selection with **-reg**, **-stat** and **-sel** options respectively. All are cleared if no option is passed
///
#[derive(Builder)]
pub struct Seqclean {
    #[builder(start_fn, into)]
    sequence: String,
    #[builder(default = false)]
    clear_registration: bool,
    #[builder(default = false)]
    clear_stats: bool,
    #[builder(default = false)]
    clear_selection: bool,
}

impl Command for Seqclean {
    fn name() -> &'static str {
        "seqclean"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.sequence.clone()),
            Argument::flag_option("reg", self.clear_registration),
            Argument::flag_option("stat", self.clear_stats),
            Argument::flag_option("sel", self.clear_selection),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_only_clears_all() {
        let cmd = Seqclean::builder("lights").build();
        assert_eq!(cmd.to_args_string(), "seqclean lights");
    }

    #[test]
    fn clear_registration_only() {
        let cmd = Seqclean::builder("lights").clear_registration(true).build();
        assert_eq!(cmd.to_args_string(), "seqclean lights -reg");
    }

    #[test]
    fn clear_stats_only() {
        let cmd = Seqclean::builder("lights").clear_stats(true).build();
        assert_eq!(cmd.to_args_string(), "seqclean lights -stat");
    }

    #[test]
    fn clear_selection_only() {
        let cmd = Seqclean::builder("lights").clear_selection(true).build();
        assert_eq!(cmd.to_args_string(), "seqclean lights -sel");
    }

    #[test]
    fn clear_all_flags_explicit() {
        let cmd = Seqclean::builder("lights")
            .clear_registration(true)
            .clear_stats(true)
            .clear_selection(true)
            .build();
        assert_eq!(cmd.to_args_string(), "seqclean lights -reg -stat -sel");
    }

    #[test]
    fn false_flags_omitted() {
        let cmd = Seqclean::builder("lights")
            .clear_registration(false)
            .clear_stats(false)
            .clear_selection(false)
            .build();
        assert_eq!(cmd.to_args_string(), "seqclean lights");
    }

    #[test]
    fn sequence_name_with_spaces_is_quoted() {
        let cmd = Seqclean::builder("my sequence").build();
        assert_eq!(cmd.to_args_string(), "seqclean 'my sequence'");
    }
}
