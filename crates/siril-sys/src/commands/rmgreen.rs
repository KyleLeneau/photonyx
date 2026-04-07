use bon::Builder;

use crate::{
    RmgreenProtection,
    commands::{Argument, Command},
};

/// ```text
/// rmgreen [-nopreserve] [type] [amount]
/// ```
///
/// Applies a chromatic noise reduction filter. It removes green tint in the current image. This filter is based on PixInsight's SCNR and it is also the same filter used by HLVG plugin in Photoshop.
/// Lightness is preserved by default but this can be disabled with the **-nopreserve** switch.
///
/// **Type** can take values 0 for average neutral, 1 for maximum neutral, 2 for maximum mask, 3 for additive mask, defaulting to 0. The last two can take an **amount** argument, a value between 0 and 1, defaulting to 1
///
#[derive(Builder)]
pub struct Rmgreen {
    #[builder(default = false)]
    nopreserve: bool,
    protection: Option<RmgreenProtection>,
    amount: Option<f64>,
}

impl Command for Rmgreen {
    fn name() -> &'static str {
        "rmgreen"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::flag_option("nopreserve", self.nopreserve),
            Argument::positional_option(self.protection),
            Argument::positional_option(self.amount),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_no_args() {
        let cmd = Rmgreen::builder().build();
        assert_eq!(cmd.to_args_string(), "rmgreen");
    }

    #[test]
    fn nopreserve_flag() {
        let cmd = Rmgreen::builder().nopreserve(true).build();
        assert_eq!(cmd.to_args_string(), "rmgreen -nopreserve");
    }

    #[test]
    fn nopreserve_false_omitted() {
        let cmd = Rmgreen::builder().nopreserve(false).build();
        assert!(!cmd.to_args_string().contains("nopreserve"));
    }

    #[test]
    fn protection_average_neutral() {
        let cmd = Rmgreen::builder()
            .protection(RmgreenProtection::AverageNeutral)
            .build();
        assert_eq!(cmd.to_args_string(), "rmgreen AverageNeutral");
    }

    #[test]
    fn protection_maximum_neutral() {
        let cmd = Rmgreen::builder()
            .protection(RmgreenProtection::MaximumNeutral)
            .build();
        assert_eq!(cmd.to_args_string(), "rmgreen MaximumNeutral");
    }

    #[test]
    fn protection_maximum_mask() {
        let cmd = Rmgreen::builder()
            .protection(RmgreenProtection::MaximumMask)
            .build();
        assert_eq!(cmd.to_args_string(), "rmgreen MaximumMask");
    }

    #[test]
    fn protection_additive_mask() {
        let cmd = Rmgreen::builder()
            .protection(RmgreenProtection::AdditiveMask)
            .build();
        assert_eq!(cmd.to_args_string(), "rmgreen AdditiveMask");
    }

    #[test]
    fn protection_with_amount() {
        let cmd = Rmgreen::builder()
            .protection(RmgreenProtection::MaximumMask)
            .amount(0.5)
            .build();
        assert_eq!(cmd.to_args_string(), "rmgreen MaximumMask 0.5");
    }

    #[test]
    fn amount_without_protection_still_emits() {
        let cmd = Rmgreen::builder().amount(0.75).build();
        assert_eq!(cmd.to_args_string(), "rmgreen 0.75");
    }

    #[test]
    fn nopreserve_with_protection_and_amount() {
        let cmd = Rmgreen::builder()
            .nopreserve(true)
            .protection(RmgreenProtection::AdditiveMask)
            .amount(0.8)
            .build();
        assert_eq!(cmd.to_args_string(), "rmgreen -nopreserve AdditiveMask 0.8");
    }
}
