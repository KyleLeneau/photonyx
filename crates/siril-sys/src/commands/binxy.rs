use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// binxy coefficient [-sum]
/// ```
///
/// Computes the numerical binning of the in-memory image (sum of the pixels 2x2, 3x3..., like the analogic binning of CCD camera). If the optional argument **-sum** is passed, then the sum of pixels is computed, while it is the average when no optional argument is provided
///
#[derive(Builder)]
pub struct Binxy {
    #[builder(start_fn)]
    coefficient: f32,
    #[builder(default = false)]
    sum: bool,
}

impl Command for Binxy {
    fn name() -> &'static str {
        "binxy"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.coefficient.to_string()),
            Argument::flag_option("sum", self.sum),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coefficient_only() {
        let cmd = Binxy::builder(2.0_f32).build();
        assert_eq!(cmd.to_args_string(), "binxy 2");
    }

    #[test]
    fn with_sum_flag() {
        let cmd = Binxy::builder(3.0_f32).sum(true).build();
        assert_eq!(cmd.to_args_string(), "binxy 3 -sum");
    }

    #[test]
    fn sum_false_omitted() {
        let cmd = Binxy::builder(4.0_f32).sum(false).build();
        assert_eq!(cmd.to_args_string(), "binxy 4");
    }
}
