use anyhow::Result;
use px_cli::CalibrateProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn calibrate_project(
    _args: CalibrateProjectArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
