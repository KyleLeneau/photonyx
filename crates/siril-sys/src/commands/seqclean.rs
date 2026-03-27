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
pub struct Seqclean {}

impl Command for Seqclean {
    fn name() -> &'static str {
        "seqclean"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}

// TODO: Need command implementation
