use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqsubsky sequencename { -rbf | degree } [-nodither] [-samples=20] [-tolerance=1.0] [-smooth=0.5] [-prefix=]
/// ```
///
/// Same command as SUBSKY but for the sequence **sequencename**.
/// Dithering, required for low dynamic gradients, can be disabled with **-nodither**. Note that the **-existing** option is not available for sequence background removal, as the frames of a sequence are not necessarily always aligned.
///
/// The output sequence name starts with the prefix "bkg\_" unless otherwise specified with **-prefix=** option. Only selected images in the sequence are processed
///
/// Links: :ref:`subsky <subsky>`
///
#[derive(Builder)]
pub struct SeqSubSky {
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
    #[builder(into)]
    prefix: Option<String>,
}

impl Command for SeqSubSky {
    fn name() -> &'static str {
        "seqsubsky"
    }

    fn args(&self) -> Vec<Argument> {
        let mut args = vec![Argument::positional(&self.sequence)];

        if self.use_rbf {
            args.push(Argument::flag_option("rbf", self.use_rbf));
            args.push(Argument::option("smooth", self.smooth));
        } else {
            args.push(Argument::positional(self.degree.to_string()));
        }

        args.extend([
            Argument::flag_option("nodither", self.dither),
            Argument::option("samples", self.samples),
            Argument::option("tolerance", self.tolerance),
            Argument::option("prefix", self.prefix.as_deref()),
        ]);

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_degree_one() {
        let cmd = SeqSubSky::builder("lights").build();
        assert_eq!(cmd.to_args_string(), "seqsubsky lights 1");
    }

    #[test]
    fn custom_degree() {
        let cmd = SeqSubSky::builder("lights").degree(2).build();
        assert_eq!(cmd.to_args_string(), "seqsubsky lights 2");
    }

    #[test]
    fn rbf_replaces_degree() {
        let cmd = SeqSubSky::builder("lights").use_rbf(true).build();
        let s = cmd.to_args_string();
        assert!(s.contains("-rbf"));
        assert!(!s.contains(" 1"));
    }

    #[test]
    fn rbf_with_smooth() {
        let cmd = SeqSubSky::builder("lights")
            .use_rbf(true)
            .smooth(0.5_f32)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-rbf"));
        assert!(s.contains("-smooth=0.5"));
    }

    #[test]
    fn nodither_flag() {
        let cmd = SeqSubSky::builder("lights").dither(true).build();
        assert!(cmd.to_args_string().contains("-nodither"));
    }

    #[test]
    fn samples_and_tolerance() {
        let cmd = SeqSubSky::builder("lights")
            .samples(30)
            .tolerance(1.5_f32)
            .build();
        let s = cmd.to_args_string();
        assert!(s.contains("-samples=30"));
        assert!(s.contains("-tolerance=1.5"));
    }

    #[test]
    fn custom_prefix() {
        let cmd = SeqSubSky::builder("lights").prefix("bkg2_").build();
        assert!(cmd.to_args_string().contains("-prefix=bkg2_"));
    }
}
