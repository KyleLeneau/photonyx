use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// requires min_version [obsolete_version]
/// ```
///
/// Returns an error if the version of Siril is older than the minimum required version passed in the first argument. Optionally, takes a second argument for the Siril version at which the script is obsolete: returns an error if the version of Siril is **newer than or equal to** the one passed in the second argument.
///
/// Example: *requires 1.2.0 1.4.0* allows the script to run for all of the 1.2.x series and 1.3.x series, but will not run for any versions earlier than 1.2.0 or for version 1.4.0 or any later versions
///
#[derive(Builder)]
pub struct Requires {
    #[builder(start_fn)]
    min_version: String,
    obsolete_version: Option<String>
}

impl Command for Requires {
    fn name() -> &'static str {
        "requires"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.min_version.to_string()),
            Argument::positional_option(self.obsolete_version.clone())
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_version_only() {
        let cmd = Requires::builder("1.2.0".to_string()).build();
        assert_eq!(cmd.to_args_string(), "requires 1.2.0");
    }

    #[test]
    fn min_and_obsolete_version() {
        let cmd = Requires::builder("1.2.0".to_string())
            .obsolete_version("1.4.0".to_string())
            .build();
        assert_eq!(cmd.to_args_string(), "requires 1.2.0 1.4.0");
    }
}
