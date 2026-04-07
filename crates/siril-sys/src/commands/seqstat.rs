use bon::Builder;

use crate::{
    StatOption,
    commands::{Argument, Command},
};

/// ```text
/// seqstat sequencename output_file [option] [-cfa]
/// ```
///
/// Same command as STAT for sequence **sequencename**.
///
/// Data is saved as a csv file **output_file**.
/// The optional parameter defines the number of statistical values computed: **basic**, **main** (default) or **full** (more detailed but longer to compute).
/// \\t\ **basic** includes mean, median, sigma, bgnoise, min and max
/// \\t\ **main** includes basic with the addition of avgDev, MAD and the square root of BWMV
/// \\t\ **full** includes main with the addition of location and scale.
///
/// If **-cfa** is passed and the images are CFA, statistics are made on per-filter extractions
///
/// Links: :ref:`stat <stat>`
///
#[derive(Builder)]
pub struct Seqstat {
    #[builder(start_fn)]
    sequence: String,
    #[builder(start_fn, into)]
    output_csv: String,
    stats: Option<StatOption>,
    #[builder(default = false)]
    by_cfa: bool,
}

impl Command for Seqstat {
    fn name() -> &'static str {
        "seqstat"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.sequence.clone()),
            Argument::positional(self.output_csv.clone()),
            Argument::positional_option(self.stats),
            Argument::flag_option("cfa", self.by_cfa),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_and_output_only() {
        let cmd = Seqstat::builder("lights".to_string(), "stats.csv").build();
        assert_eq!(cmd.to_args_string(), "seqstat lights stats.csv");
    }

    #[test]
    fn stat_option_basic() {
        let cmd = Seqstat::builder("lights".to_string(), "stats.csv")
            .stats(StatOption::Basic)
            .build();
        assert_eq!(cmd.to_args_string(), "seqstat lights stats.csv basic");
    }

    #[test]
    fn stat_option_main() {
        let cmd = Seqstat::builder("lights".to_string(), "stats.csv")
            .stats(StatOption::Main)
            .build();
        assert_eq!(cmd.to_args_string(), "seqstat lights stats.csv main");
    }

    #[test]
    fn stat_option_full() {
        let cmd = Seqstat::builder("lights".to_string(), "stats.csv")
            .stats(StatOption::Full)
            .build();
        assert_eq!(cmd.to_args_string(), "seqstat lights stats.csv full");
    }

    #[test]
    fn cfa_flag() {
        let cmd = Seqstat::builder("lights".to_string(), "stats.csv")
            .by_cfa(true)
            .build();
        assert_eq!(cmd.to_args_string(), "seqstat lights stats.csv -cfa");
    }

    #[test]
    fn cfa_false_omitted() {
        let cmd = Seqstat::builder("lights".to_string(), "stats.csv")
            .by_cfa(false)
            .build();
        assert!(!cmd.to_args_string().contains("cfa"));
    }

    #[test]
    fn stat_option_with_cfa() {
        let cmd = Seqstat::builder("lights".to_string(), "stats.csv")
            .stats(StatOption::Full)
            .by_cfa(true)
            .build();
        assert_eq!(cmd.to_args_string(), "seqstat lights stats.csv full -cfa");
    }
}
