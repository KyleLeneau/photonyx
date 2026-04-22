use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// pm "expression" [-rescale [low] [high]] [-nosum]
/// ```
///
/// This command evaluates the expression given in argument as in PixelMath tool. The full expression must be between double quotes and variables (that are image names, without extension, located in the working directory in that case) must be surrounded by the token $, e.g. "$ \* 0.5 + $ \* 0.5". A maximum of 10 images can be used in the expression.
/// Image can be rescaled with the option **-rescale** followed by **low** and **high** values in the range [0, 1]. If no low and high values are provided, default values are set to 0 and 1. Another optional argument, **-nosum** tells Siril not to sum exposure times. This impacts FITS keywords such as LIVETIME and STACKCNT
///
#[derive(Builder)]
pub struct Pm {
    #[builder(start_fn, into)]
    expression: String,
    #[builder(default = false)]
    rescale: bool,
    rescale_low: Option<f32>,
    rescale_high: Option<f32>,
    #[builder(default = false)]
    no_sum: bool,
}

impl Command for Pm {
    fn name() -> &'static str {
        "pm"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![Argument::double_quoted(&self.expression)];

        if self.rescale {
            args.push(Argument::flag("rescale"));
            args.push(Argument::positional_option(self.rescale_low));
            args.push(Argument::positional_option(self.rescale_high));
        }
        args.push(Argument::flag_option("nosum", self.no_sum));

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal() {
        let cmd = Pm::builder("$img1$*0.5").build();
        assert_eq!(cmd.to_args_string(), r#"pm "$img1$*0.5""#);
    }

    #[test]
    fn expression_with_spaces_is_double_quoted() {
        let cmd = Pm::builder("$img1$ * 0.5").build();
        assert_eq!(cmd.to_args_string(), r#"pm "$img1$ * 0.5""#);
    }

    #[test]
    fn no_sum() {
        let cmd = Pm::builder("$img1$ * 0.5").no_sum(true).build();
        assert_eq!(cmd.to_args_string(), r#"pm "$img1$ * 0.5" -nosum"#);
    }

    #[test]
    fn rescale_no_low_high() {
        let cmd = Pm::builder("$img1$ * 0.5").rescale(true).build();
        assert_eq!(cmd.to_args_string(), r#"pm "$img1$ * 0.5" -rescale"#);
    }

    #[test]
    fn rescale_with_low_and_high() {
        let cmd = Pm::builder("$img1$ * 0.5")
            .rescale(true)
            .rescale_low(0.0_f32)
            .rescale_high(1.0_f32)
            .build();
        assert_eq!(cmd.to_args_string(), r#"pm "$img1$ * 0.5" -rescale 0 1"#);
    }

    #[test]
    fn rescale_with_low_and_high_and_no_sum() {
        let cmd = Pm::builder("$a$ * 0.5 + $b$ * 0.5")
            .rescale(true)
            .rescale_low(0.1_f32)
            .rescale_high(0.9_f32)
            .no_sum(true)
            .build();
        assert_eq!(
            cmd.to_args_string(),
            r#"pm "$a$ * 0.5 + $b$ * 0.5" -rescale 0.1 0.9 -nosum"#
        );
    }
}
