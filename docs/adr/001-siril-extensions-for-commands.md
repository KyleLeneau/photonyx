# Siril Extensions for Commands

Extension trait per command module (decentralized)
Since Siril lives in the same crate, you can impl a trait for it directly in each command file:


// commands/calibrate.rs
pub trait CalibrateExt {
    async fn calibrate(&mut self, cmd: &Calibrate) -> Result<Vec<String>, SirilError>;
}

impl CalibrateExt for Siril {
    async fn calibrate(&mut self, cmd: &Calibrate) -> Result<Vec<String>, SirilError> {
        self.execute(cmd).await
    }
}
Each new command module is completely self-contained — no central file ever needs to be touched. The downside is that callers must import each trait, which is annoying unless you add a prelude:


// commands/prelude.rs (or lib.rs)
pub use crate::commands::calibrate::CalibrateExt;
pub use crate::commands::stack::StackExt;
// ...

// Caller just does:
use siril_sys::commands::prelude::*;
siril.calibrate(&cmd).await?;
The prelude does need to be updated per command, but it's a one-liner addition and much less invasive than adding a method to Siril itself.
