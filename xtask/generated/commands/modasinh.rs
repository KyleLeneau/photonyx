use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// modasinh -D= [-LP=] [-SP=] [-HP=] [-clipmode=] [-human | -even | -independent | -sat] [channels]
/// ```
///
/// Modified arcsinh stretch based on the work of the ghsastro.co.uk team.
///
/// The argument **-D=** defines the strength of the stretch, between 0 and 10. This is the only mandatory argument. The following optional arguments further tailor the stretch:
/// **LP** defines a shadow preserving range between 0 and SP where the stretch will be linear, preserving shadow detail;
/// **SP** defines the symmetry point of the stretch, between 0 and 1, which is the point at which the stretch will be most intense;
/// **HP** defines a region between HP and 1 where the stretch is linear, preserving highlight details and preventing star bloat.
/// If omitted LP and SP default to 0.0 ad HP defaults to 1.0.
/// An optional argument (either **-human**, **-even** or **-independent**) can be passed to select either human-weighted or even-weighted luminance or independent colour channels for colour stretches. The argument is ignored for mono images. Alternatively, the argument **-sat** specifies that the stretch is performed on image saturation - the image must be color and all channels must be selected for this to work.
/// Optionally the parameter **[channels]** may be used to specify the channels to apply the stretch to: this may be R, G, B, RG, RB or GB. The default is all channels. The clip mode can be set using the argument **-clipmode=**: values **clip**, **rescale**, **rgbblend** or **globalrescale** are accepted and the default is rgbblend
///
#[derive(Builder)]
pub struct Modasinh {}

impl Command for Modasinh {
    fn name() -> &'static str {
        "modasinh"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
