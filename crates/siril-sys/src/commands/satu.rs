use bon::Builder;

use crate::{
    SaturationHueRange,
    commands::{Argument, Command},
};

/// ```text
/// satu amount [background_factor [hue_range_index]]
/// ```
///
/// Enhances the color saturation of the loaded image. Try iteratively to obtain best results.
/// **amount** can be a positive number to increase color saturation, negative to decrease it, 0 would do nothing, 1 would increase it by 100%
/// **background_factor** is a factor to (median + sigma) used to set a threshold for which only pixels above it would be modified. This allows background noise to not be color saturated, if chosen carefully. Defaults to 1. Setting 0 disables the threshold.
/// **hue_range_index** can be [0, 6], meaning: 0 for pink to orange, 1 for orange to yellow, 2 for yellow to cyan, 3 for cyan, 4 for cyan to magenta, 5 for magenta to pink, 6 for all (default)
///
#[derive(Builder)]
pub struct Satu {
    #[builder(start_fn)]
    amount: f64,
    background_factor: Option<f64>,
    hue_range: Option<SaturationHueRange>,
}

impl Command for Satu {
    fn name() -> &'static str {
        "satu"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![Argument::positional(format!("{:1.2}", self.amount))];

        if self.background_factor.is_some() {
            args.push(Argument::positional_option(self.background_factor));
            if self.hue_range.is_some() {
                args.push(Argument::positional_option(self.hue_range));
            }
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amount_only() {
        let cmd = Satu::builder(1.0).build();
        assert_eq!(cmd.to_args_string(), "satu 1.00");
    }

    #[test]
    fn amount_formatted_to_two_decimal_places() {
        let cmd = Satu::builder(0.5).build();
        assert_eq!(cmd.to_args_string(), "satu 0.50");
    }

    #[test]
    fn negative_amount() {
        let cmd = Satu::builder(-0.75).build();
        assert_eq!(cmd.to_args_string(), "satu -0.75");
    }

    #[test]
    fn amount_with_background_factor() {
        let cmd = Satu::builder(1.0).background_factor(2.0).build();
        assert_eq!(cmd.to_args_string(), "satu 1.00 2");
    }

    #[test]
    fn background_factor_zero_disables_threshold() {
        let cmd = Satu::builder(1.0).background_factor(0.0).build();
        assert_eq!(cmd.to_args_string(), "satu 1.00 0");
    }

    #[test]
    fn hue_range_ignored_without_background_factor() {
        let cmd = Satu::builder(1.0)
            .hue_range(SaturationHueRange::Cyan)
            .build();
        // hue_range requires background_factor to be emitted
        assert_eq!(cmd.to_args_string(), "satu 1.00");
    }

    #[test]
    fn all_three_args() {
        let cmd = Satu::builder(0.5)
            .background_factor(1.0)
            .hue_range(SaturationHueRange::PinkOrange)
            .build();
        assert_eq!(cmd.to_args_string(), "satu 0.50 1 PinkOrange");
    }

    #[test]
    fn hue_range_all() {
        let cmd = Satu::builder(2.0)
            .background_factor(1.5)
            .hue_range(SaturationHueRange::ALL)
            .build();
        assert_eq!(cmd.to_args_string(), "satu 2.00 1.5 ALL");
    }

    #[test]
    fn hue_range_magenta_pink() {
        let cmd = Satu::builder(1.0)
            .background_factor(1.0)
            .hue_range(SaturationHueRange::MagentaPink)
            .build();
        assert_eq!(cmd.to_args_string(), "satu 1.00 1 MagentaPink");
    }
}
