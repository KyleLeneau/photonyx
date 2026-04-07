use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// save filename [-chksum]
/// ```
///
/// Saves current image to **filename**.fit (or .fits, depending on your preferences, see SETEXT) in the current working directory. The image remains loaded. **filename** can contain a path as long as the directory already exists. The **-chksum** option stores checksum keywords (CHECKSUM and DATASUM) in the FITS header
///
/// Links: :ref:`setext <setext>`
///
#[derive(Builder)]
pub struct Save {
    #[builder(start_fn, into)]
    filename: String,
    #[builder(default = false)]
    chksum: bool,
}

impl Command for Save {
    fn name() -> &'static str {
        "save"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.filename),
            Argument::flag_option("chksum", self.chksum),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_only() {
        let cmd = Save::builder("result").build();
        assert_eq!(cmd.to_args_string(), "save result");
    }

    #[test]
    fn filename_with_chksum_flag() {
        let cmd = Save::builder("result").chksum(true).build();
        assert_eq!(cmd.to_args_string(), "save result -chksum");
    }

    #[test]
    fn chksum_false_omits_flag() {
        let cmd = Save::builder("result").chksum(false).build();
        assert_eq!(cmd.to_args_string(), "save result");
    }

    #[test]
    fn filename_with_spaces_is_quoted() {
        let cmd = Save::builder("my result").build();
        assert_eq!(cmd.to_args_string(), "save 'my result'");
    }
}
