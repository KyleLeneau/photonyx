use anyhow::Result;
use px_cli::ListMasterArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn list_masters(_args: ListMasterArgs, printer: Printer) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
