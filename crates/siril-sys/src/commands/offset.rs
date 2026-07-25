use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// offset value
/// ```
///
/// Adds the constant **value** (specified in ADU) to the current image. This constant can take a negative value.
///
/// In 16-bit mode, values of pixels that fall outside of [0, 65535] are clipped. In 32-bit mode, no clipping occurs
///
#[derive(Builder)]
pub struct Offset {
    #[builder(start_fn)]
    value: f32,
}

impl Command for Offset {
    fn name() -> &'static str {
        "offset"
    }

    fn args(&self) -> Vec<Argument> {
        vec![Argument::positional(self.value.to_string())]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_value() {
        let cmd = Offset::builder(1.0_f32).build();
        assert_eq!(cmd.to_args_string(), "offset 1");
    }

    #[test]
    fn with_fractional_level() {
        let cmd = Offset::builder(0.001_f32).build();
        assert_eq!(cmd.to_args_string(), "offset 0.001");
    }
}
