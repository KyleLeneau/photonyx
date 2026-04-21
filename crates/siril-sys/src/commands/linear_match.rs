use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// linear_match reference low high
/// ```
///
/// Computes and applies a linear function between a **reference** image and the loaded image.
///
/// The algorithm will ignore all reference pixels whose values are outside of the [**low**, **high**] range
///
#[derive(Builder)]
pub struct LinearMatch {
    #[builder(start_fn, into)]
    reference: String,
    #[builder(start_fn)]
    low: f32,
    #[builder(start_fn)]
    high: f32,
}

impl Command for LinearMatch {
    fn name() -> &'static str {
        "linear_match"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(&self.reference),
            Argument::positional(self.low.to_string()),
            Argument::positional(self.high.to_string()),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_args() {
        let cmd = LinearMatch::builder("reference.fits", 0.0f32, 1.0f32).build();
        assert_eq!(cmd.to_args_string(), "linear_match reference.fits 0 1");
    }

    #[test]
    fn fractional_range() {
        let cmd = LinearMatch::builder("ref.fits", 0.1f32, 0.9f32).build();
        assert_eq!(cmd.to_args_string(), "linear_match ref.fits 0.1 0.9");
    }

    #[test]
    fn reference_with_spaces_is_quoted() {
        let cmd = LinearMatch::builder("my reference.fits", 0.0f32, 1.0f32).build();
        assert_eq!(cmd.to_args_string(), "linear_match 'my reference.fits' 0 1");
    }
}
