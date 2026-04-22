use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// link basename [-date] [-start=index] [-out=]
/// ```
///
/// Same as CONVERT but converts only FITS files found in the current working directory. This is useful to avoid conversions of JPEG results or other files that may end up in the directory. The additional argument **-date** enables sorting files with their DATE-OBS value instead of with their name alphanumerically
///
/// Links: :ref:`convert <convert>`
///
#[derive(Builder)]
pub struct Link {
    #[builder(start_fn, into)]
    basename: String,
    #[builder(default = false)]
    sort_by_obs_date: bool,
    output: Option<String>,
}

impl Command for Link {
    fn name() -> &'static str {
        "link"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.basename),
            Argument::flag_option("date", self.sort_by_obs_date),
            Argument::option("out", self.output.as_ref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal() {
        let cmd = Link::builder("sequence").build();
        assert_eq!(cmd.to_args_string(), "link sequence");
    }

    #[test]
    fn sort_by_obs_date() {
        let cmd = Link::builder("sequence").sort_by_obs_date(true).build();
        assert_eq!(cmd.to_args_string(), "link sequence -date");
    }

    #[test]
    fn with_output() {
        let cmd = Link::builder("sequence").output("out_dir".to_string()).build();
        assert_eq!(cmd.to_args_string(), "link sequence -out=out_dir");
    }

    #[test]
    fn all_options() {
        let cmd = Link::builder("sequence")
            .sort_by_obs_date(true)
            .output("out_dir".to_string())
            .build();
        assert_eq!(cmd.to_args_string(), "link sequence -date -out=out_dir");
    }
}
