use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqheader sequencename keyword [keyword2 ...] [-sel] [-out=file.csv]
/// ```
///
/// Prints the FITS header value corresponding to the given keys for all images in the sequence. You can write several keys in a row, separated by a space. The **-out=** option, followed by a file name, allows you to print the output in a csv file. The **-sel** option limits the output to the images selected in the sequence
///
#[derive(Builder)]
pub struct Seqheader {
    #[builder(start_fn)]
    sequence: String,
    #[builder(start_fn)]
    keyword: String,
    #[builder(default)]
    extra_keywords: Vec<String>,
    #[builder(default = false)]
    only_selected: bool,
    #[builder(into)]
    output_csv: Option<String>,
}

impl Command for Seqheader {
    fn name() -> &'static str {
        "seqheader"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![
            Argument::positional(self.sequence.clone()),
            Argument::positional(self.keyword.clone()),
        ];

        for extra in &self.extra_keywords {
            args.push(Argument::positional(extra));
        }

        args.push(Argument::flag_option("sel", self.only_selected));
        args.push(Argument::option("out", self.output_csv.clone()));
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequence_and_keyword_only() {
        let cmd = Seqheader::builder("lights".to_string(), "EXPTIME".to_string()).build();
        assert_eq!(cmd.to_args_string(), "seqheader lights EXPTIME");
    }

    #[test]
    fn extra_keywords() {
        let cmd = Seqheader::builder("lights".to_string(), "EXPTIME".to_string())
            .extra_keywords(vec!["DATE-OBS".to_string(), "OBJECT".to_string()])
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "seqheader lights EXPTIME DATE-OBS OBJECT"
        );
    }

    #[test]
    fn only_selected_flag() {
        let cmd = Seqheader::builder("lights".to_string(), "EXPTIME".to_string())
            .only_selected(true)
            .build();
        assert_eq!(cmd.to_args_string(), "seqheader lights EXPTIME -sel");
    }

    #[test]
    fn only_selected_false_omitted() {
        let cmd = Seqheader::builder("lights".to_string(), "EXPTIME".to_string())
            .only_selected(false)
            .build();
        assert!(!cmd.to_args_string().contains("sel"));
    }

    #[test]
    fn output_csv() {
        let cmd = Seqheader::builder("lights".to_string(), "EXPTIME".to_string())
            .output_csv("results.csv")
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "seqheader lights EXPTIME -out=results.csv"
        );
    }

    #[test]
    fn all_options() {
        let cmd = Seqheader::builder("lights".to_string(), "EXPTIME".to_string())
            .extra_keywords(vec!["DATE-OBS".to_string()])
            .only_selected(true)
            .output_csv("results.csv")
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "seqheader lights EXPTIME DATE-OBS -sel -out=results.csv"
        );
    }
}
