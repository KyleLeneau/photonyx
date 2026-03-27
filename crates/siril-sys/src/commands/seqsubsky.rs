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
    #[builder(start_fn)]
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
