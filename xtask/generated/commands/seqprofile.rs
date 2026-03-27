use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// seqprofile sequence -from=x,y -to=x,y [-tri] [-cfa] [-arcsec] [-savedat] [-layer=] [-width=] [-spacing=] [ {-xaxis=wavelength | -xaxis=wavenumber } ] [{-wavenumber1= | -wavelength1=} -wn1at=x,y {-wavenumber2= | -wavelength2=} -wn2at=x,y] ["-title=My Plot"]
/// ```
///
/// Generates an intensity profile plot between 2 points in each image in the sequence. After the mandatory first argument stating the sequence to process, the other arguments are the same as for the **profile** command. If processing a sequence and it is desired to have the current image number and total number of images displayed in the format "My Sequence (1 / 5)", the given title should end with () (e.g. "My Sequence ()" and the numbers will be populated automatically)
///
#[derive(Builder)]
pub struct Seqprofile {}

impl Command for Seqprofile {
    fn name() -> &'static str {
        "seqprofile"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
