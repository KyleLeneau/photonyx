use anyhow::Result;
use px_cli::CalibrateObservationArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn calibrate_observation(
    _args: CalibrateObservationArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
