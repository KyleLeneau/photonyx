use bon::Builder;

use crate::commands::{Argument, Command};

/// ```text
/// livestack filename
/// ```
///
/// Process the provided image for live stacking. Only possible after START_LS. The process involves calibrating the incoming file if configured in START_LS, demosaicing if it's an OSC image, registering and stacking. The temporary result will be in the file live_stack_00001.fit until a new option to change it is added
///
/// Links: :ref:`start_ls <start_ls>`
///
/// |
///
/// .. warning::
///
///     Note that the live stacking commands put Siril in a state in which it's not 
///     able to process other commands. After START_LS, only LIVESTACK, STOP_LS and 
///     EXIT can be called until STOP_LS is called to return Siril in its normal, 
///     non-live-stacking, state.
///
#[derive(Builder)]
pub struct Livestack {}

impl Command for Livestack {
    fn name() -> &'static str {
        "livestack"
    }

    fn args(&self) -> Vec<Argument> {
        vec![]
    }
}
