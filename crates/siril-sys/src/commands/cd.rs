use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// cd directory
/// ```
///
/// Sets the new current working directory.
///
/// The argument **directory** can contain the ~ token, expanded as the home directory, directories with spaces in the name can be protected using single or double quotes
///
#[derive(Builder)]
pub struct Cd {
    #[builder(start_fn, into)]
    directory: String,
}

impl Command for Cd {
    fn name() -> &'static str {
        "cd"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(self.directory.to_string())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_directory() {
        let cmd = Cd::builder("/home/user/photos").build();
        assert_eq!(cmd.to_args_string(), "cd /home/user/photos");
    }

    #[test]
    fn directory_with_spaces_is_quoted() {
        let cmd = Cd::builder("/home/user/my photos").build();
        assert_eq!(cmd.to_args_string(), "cd '/home/user/my photos'");
    }

    #[test]
    fn home_tilde_shorthand() {
        let cmd = Cd::builder("~/photos").build();
        assert_eq!(cmd.to_args_string(), "cd ~/photos");
    }
}
