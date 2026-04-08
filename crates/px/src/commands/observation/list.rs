use anyhow::Result;
use px_cli::ListObservationArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn list_observations(
    _args: ListObservationArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
