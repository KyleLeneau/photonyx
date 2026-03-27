use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// pcc [-limitmag=[+-]] [-catalog=] [-bgtol=lower,upper]
/// ```
///
/// Run the Photometric Color Correction on the loaded plate-solved image.
///
/// The limit magnitude of stars is automatically computed from the size of the field of view, but can be altered by passing a +offset or -offset value to **-limitmag=**, or simply an absolute positive value for the limit magnitude.
/// The star catalog used is NOMAD by default, it can be changed by providing **-catalog=apass**, **-catalog=localgaia** or **-catalog=gaia**. If installed locally, the remote NOMAD (the complete version) can be forced by providing **-catalog=nomad**
/// Background reference outlier tolerance can be specified in sigma units using **-bgtol=lower,upper**: these default to -2.8 and +2.0
///
#[derive(Builder)]
pub struct Pcc {}

impl Command for Pcc {
    fn name() -> &'static str {
        "pcc"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
