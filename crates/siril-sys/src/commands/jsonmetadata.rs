use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// jsonmetadata FITS_file [-stats_from_loaded] [-nostats] [-out=]
/// ```
///
/// Dumps metadata and statistics of the currently loaded image in JSON form. The file name is required, even if the image is already loaded. Image data may not be read from the file if it is the current loaded image and if the **-stats_from_loaded** option is passed. Statistics can be disabled by providing the **-nostats** option. A file containing the JSON data is created with default file name '$(FITS_file_without_ext).json' and can be changed with the **-out=** option
///
#[derive(Builder)]
pub struct Jsonmetadata {
    #[builder(start_fn)]
    filename: String,
    #[builder(default = false)]
    stats_from_loaded: bool,
    #[builder(default = false)]
    no_stats: bool,
    output: Option<String>,
}

impl Command for Jsonmetadata {
    fn name() -> &'static str {
        "jsonmetadata"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.filename.to_string()),
            Argument::flag_option("stats_from_loaded", self.stats_from_loaded),
            Argument::flag_option("nostats", self.no_stats),
            Argument::option("out", self.output.as_ref()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal() {
        let cmd = Jsonmetadata::builder("image.fits".to_string()).build();
        assert_eq!(cmd.to_args_string(), "jsonmetadata image.fits");
    }

    #[test]
    fn stats_from_loaded() {
        let cmd = Jsonmetadata::builder("image.fits".to_string())
            .stats_from_loaded(true)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "jsonmetadata image.fits -stats_from_loaded"
        );
    }

    #[test]
    fn no_stats() {
        let cmd = Jsonmetadata::builder("image.fits".to_string())
            .no_stats(true)
            .build();
        assert_eq!(cmd.to_args_string(), "jsonmetadata image.fits -nostats");
    }

    #[test]
    fn with_output() {
        let cmd = Jsonmetadata::builder("image.fits".to_string())
            .output("result.json".to_string())
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "jsonmetadata image.fits -out=result.json"
        );
    }

    #[test]
    fn all_options() {
        let cmd = Jsonmetadata::builder("image.fits".to_string())
            .stats_from_loaded(true)
            .no_stats(true)
            .output("result.json".to_string())
            .build();
        assert_eq!(
            cmd.to_args_string(),
            "jsonmetadata image.fits -stats_from_loaded -nostats -out=result.json"
        );
    }
}
