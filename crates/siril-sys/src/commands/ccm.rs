use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// ccm m00 m01 m02 m10 m11 m12 m20 m21 m22 [gamma]
/// ```
///
/// Applies a color conversion matrix to the current image.
///
/// There are 9 mandatory arguments corresponding to the 9 matrix elements:
///
/// m00, m01, m02
/// m10, m11, m12
/// m20, m21, m22
///
/// An additional tenth argument **[gamma]** can be provided: if it is omitted, it defaults to 1.0.
///
/// These are applied to each pixel according to the following formulae:
///
/// r' = (m00 \* r + m01 \* g + m02 \* b)^(-1/gamma)
/// g' = (m10 \* r + m11 \* g + m12 \* b)^(-1/gamma)
/// b' = (m20 \* r + m21 \* g + m22 \* b)^(-1/gamma)
///
#[allow(clippy::too_many_arguments)]
#[derive(Builder)]
pub struct Ccm {
    #[builder(start_fn)]
    m00: f32,
    #[builder(start_fn)]
    m01: f32,
    #[builder(start_fn)]
    m02: f32,
    #[builder(start_fn)]
    m10: f32,
    #[builder(start_fn)]
    m11: f32,
    #[builder(start_fn)]
    m12: f32,
    #[builder(start_fn)]
    m20: f32,
    #[builder(start_fn)]
    m21: f32,
    #[builder(start_fn)]
    m22: f32,
    gamma: Option<f32>,
}

impl Command for Ccm {
    fn name() -> &'static str {
        "ccm"
    }

    fn args(&self) -> Vec<Argument> {
        vec![
            Argument::positional(self.m00.to_string()),
            Argument::positional(self.m01.to_string()),
            Argument::positional(self.m02.to_string()),
            Argument::positional(self.m10.to_string()),
            Argument::positional(self.m11.to_string()),
            Argument::positional(self.m12.to_string()),
            Argument::positional(self.m20.to_string()),
            Argument::positional(self.m21.to_string()),
            Argument::positional(self.m22.to_string()),
            Argument::positional_option(self.gamma),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_matrix_no_gamma() {
        let cmd = Ccm::builder(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0).build();
        assert_eq!(cmd.to_args_string(), "ccm 1 0 0 0 1 0 0 0 1");
    }

    #[test]
    fn with_gamma() {
        let cmd = Ccm::builder(1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0)
            .gamma(2.2_f32)
            .build();
        assert_eq!(cmd.to_args_string(), "ccm 1 0 0 0 1 0 0 0 1 2.2");
    }

    #[test]
    fn custom_matrix_no_gamma() {
        let cmd = Ccm::builder(1.1, 0.2, -0.3, -0.1, 0.9, 0.2, 0.0, 0.1, 0.9).build();
        assert_eq!(
            cmd.to_args_string(),
            "ccm 1.1 0.2 -0.3 -0.1 0.9 0.2 0 0.1 0.9"
        );
    }
}
