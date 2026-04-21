use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// subsky { -rbf | degree } [-dither] [-samples=20] [-tolerance=1.0] [-smooth=0.5] [-existing]
/// ```
///
/// Computes a synthetic background gradient using either the polynomial function model of **degree** degrees or the RBF model (if **-rbf** is provided instead) and subtracts it from the image.
/// The number of samples per horizontal line and the tolerance to exclude brighter areas can be adjusted with the optional arguments. Tolerance is in MAD units: median + tolerance \* mad.
/// Dithering, required for low dynamic gradients, can be enabled with **-dither**.
/// For RBF, the additional smoothing parameter is also available. To use pre-existing background samples (e.g. if you have set background samples using a Python script) the **-existing** argument must be used
///
#[derive(Builder)]
pub struct Subsky {
    #[builder(start_fn, into)]
    sequence: String,
    #[builder(default = false)]
    use_rbf: bool,
    #[builder(default = 1)]
    degree: u8,
    #[builder(default = false)]
    dither: bool,
    samples: Option<u32>,
    tolerance: Option<f32>,
    smooth: Option<f32>,
    #[builder(default = false)]
    use_existing: bool,
}

impl Command for Subsky {
    fn name() -> &'static str {
        "subsky"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![Argument::positional(&self.sequence)];

        if self.use_rbf {
            args.push(Argument::flag_option("rbf", self.use_rbf));
            args.push(Argument::option("smooth", self.smooth));
            args.push(Argument::flag_option("existing", self.use_existing));
        } else {
            args.push(Argument::positional(self.degree.to_string()));
        }

        args.extend([
            Argument::flag_option("dither", self.dither),
            Argument::option("samples", self.samples),
            Argument::option("tolerance", self.tolerance),
        ]);

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_degree_one() {
        let cmd = Subsky::builder("lights").build();
        assert_eq!(cmd.to_args_string(), "subsky lights 1");
    }

    #[test]
    fn custom_degree() {
        let cmd = Subsky::builder("lights").degree(3).build();
        assert_eq!(cmd.to_args_string(), "subsky lights 3");
    }

    #[test]
    fn rbf_replaces_degree() {
        let cmd = Subsky::builder("lights").use_rbf(true).build();
        let s = cmd.to_args_string();
        assert!(s.contains("-rbf"));
        assert!(!s.contains(" 1"));
    }

    #[test]
    fn rbf_with_smooth() {
        let cmd = Subsky::builder("lights")
            .use_rbf(true)
            .smooth(0.5_f32)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-rbf"));
        assert!(s.contains("-smooth=0.5"));
    }

    #[test]
    fn rbf_with_existing() {
        let cmd = Subsky::builder("lights")
            .use_rbf(true)
            .use_existing(true)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-rbf"));
        assert!(s.contains("-existing"));
    }

    #[test]
    fn existing_ignored_without_rbf() {
        let cmd = Subsky::builder("lights").use_existing(true).build();
        let s = cmd.to_args_string();
        assert!(!s.contains("-existing"));
    }

    #[test]
    fn smooth_ignored_without_rbf() {
        let cmd = Subsky::builder("lights").smooth(0.5_f32).build();
        let s = cmd.to_args_string();
        assert!(!s.contains("-smooth"));
    }

    #[test]
    fn dither_flag() {
        let cmd = Subsky::builder("lights").dither(true).build();
        assert!(cmd.to_args_string().contains("-dither"));
    }

    #[test]
    fn samples_and_tolerance() {
        let cmd = Subsky::builder("lights")
            .samples(30)
            .tolerance(1.5_f32)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-samples=30"));
        assert!(s.contains("-tolerance=1.5"));
    }

    #[test]
    fn all_options_polynomial() {
        let cmd = Subsky::builder("lights")
            .degree(2)
            .dither(true)
            .samples(20)
            .tolerance(1.0_f32)
            .build();
        let s = cmd.to_args_string();
        assert!(s.starts_with("subsky lights 2"));
        assert!(s.contains("-dither"));
        assert!(s.contains("-samples=20"));
        assert!(s.contains("-tolerance=1"));
    }

    #[test]
    fn all_options_rbf() {
        let cmd = Subsky::builder("lights")
            .use_rbf(true)
            .smooth(0.5_f32)
            .use_existing(true)
            .dither(true)
            .samples(20)
            .tolerance(1.0_f32)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-rbf"));
        assert!(s.contains("-smooth=0.5"));
        assert!(s.contains("-existing"));
        assert!(s.contains("-dither"));
        assert!(s.contains("-samples=20"));
        assert!(s.contains("-tolerance=1"));
    }
}
