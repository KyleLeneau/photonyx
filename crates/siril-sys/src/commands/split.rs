use bon::Builder;

use crate::{
    SplitOption,
    commands::{Argument, Command},
};

/// ```text
/// split file1 file2 file3 [-hsl | -hsv | -lab]
/// ```
///
/// Splits the loaded color image into three distinct files (one for each color) and saves them in **file1**.fit, **file2**.fit and **file3**.fit files. A last argument can optionally be supplied, **-hsl**, **-hsv** or **lab** to perform an HSL, HSV or CieLAB extraction. If no option are provided, the extraction is of RGB type, meaning no conversion is done
///
#[derive(Builder)]
pub struct Split {
    #[builder(start_fn, into)]
    file1: String,
    #[builder(start_fn, into)]
    file2: String,
    #[builder(start_fn, into)]
    file3: String,
    method: Option<SplitOption>,
}

impl Command for Split {
    fn name() -> &'static str {
        "split"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![
            Argument::positional(&self.file1),
            Argument::positional(&self.file2),
            Argument::positional(&self.file3),
        ];

        if let Some(method) = &self.method {
            args.push(Argument::flag_option(method.to_string(), true));
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_files_no_method() {
        let cmd = Split::builder("red", "green", "blue").build();
        assert_eq!(cmd.to_args_string(), "split red green blue");
    }

    #[test]
    fn hsl_method() {
        let cmd = Split::builder("h", "s", "l")
            .method(SplitOption::Hsl)
            .build();
        assert_eq!(cmd.to_args_string(), "split h s l -hsl");
    }

    #[test]
    fn hsv_method() {
        let cmd = Split::builder("h", "s", "v")
            .method(SplitOption::Hsv)
            .build();
        assert_eq!(cmd.to_args_string(), "split h s v -hsv");
    }

    #[test]
    fn lab_method() {
        let cmd = Split::builder("l", "a", "b")
            .method(SplitOption::Lab)
            .build();
        assert_eq!(cmd.to_args_string(), "split l a b -lab");
    }

    #[test]
    fn filenames_with_spaces_are_quoted() {
        let cmd = Split::builder("my red", "my green", "my blue").build();
        assert_eq!(cmd.to_args_string(), "split 'my red' 'my green' 'my blue'");
    }
}
