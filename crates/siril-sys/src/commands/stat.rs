use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// stat [-cfa] [main]
/// ```
///
/// Returns statistics of the current image, the basic list by default or the main list if **main** is passed. If a selection is made, statistics are computed within the selection. If **-cfa** is passed and the image is CFA, statistics are made on per-filter extractions
///
#[derive(Builder)]
pub struct Stat {
    #[builder(default = false)]
    by_cfa: bool,
    #[builder(into)]
    main: Option<String>,
}

impl Command for Stat {
    fn name() -> &'static str {
        "stat"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::flag_option("cfa", self.by_cfa),
            Argument::positional_option(self.main.clone()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_args() {
        let cmd = Stat::builder().build();
        assert_eq!(cmd.to_args_string(), "stat");
    }

    #[test]
    fn cfa_flag() {
        let cmd = Stat::builder().by_cfa(true).build();
        assert_eq!(cmd.to_args_string(), "stat -cfa");
    }

    #[test]
    fn cfa_false_omitted() {
        let cmd = Stat::builder().by_cfa(false).build();
        assert!(!cmd.to_args_string().contains("cfa"));
    }

    #[test]
    fn main_arg() {
        let cmd = Stat::builder().main("main").build();
        assert_eq!(cmd.to_args_string(), "stat main");
    }

    #[test]
    fn cfa_and_main() {
        let cmd = Stat::builder().by_cfa(true).main("main").build();
        assert_eq!(cmd.to_args_string(), "stat -cfa main");
    }
}
