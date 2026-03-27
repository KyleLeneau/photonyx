use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// stat [-cfa] [main]
/// ```
///
/// Returns statistics of the current image, the basic list by default or the main list if **main** is passed. If a selection is made, statistics are computed within the selection. If **-cfa** is passed and the image is CFA, statistics are made on per-filter extractions
///
#[derive(Builder)]
pub struct Stat {}

impl Command for Stat {
    fn name() -> &'static str {
        "stat"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
