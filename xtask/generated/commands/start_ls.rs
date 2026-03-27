use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// start_ls [-dark=filename] [-flat=filename] [-rotate] [-32bits]
/// ```
///
/// Initializes a livestacking session, using the optional calibration files and waits for input files to be provided by the LIVESTACK command until STOP_LS is called. Default processing will use shift-only registration and 16-bit processing because it's faster, it can be changed to rotation with **-rotate** and **-32bits**
///
/// *Note that the live stacking commands put Siril in a state in which it's not able to process other commands. After START_LS, only LIVESTACK, STOP_LS and EXIT can be called until STOP_LS is called to return Siril in its normal, non-live-stacking, state*
///
/// Links: :ref:`livestack <livestack>`, :ref:`stop_ls <stop_ls>`, :ref:`exit <exit>`
///
#[derive(Builder)]
pub struct StartLs {}

impl Command for StartLs {
    fn name() -> &'static str {
        "start_ls"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
