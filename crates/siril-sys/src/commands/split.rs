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
    #[builder(start_fn)]
    file1: String,
    #[builder(start_fn)]
    file2: String,
    #[builder(start_fn)]
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
