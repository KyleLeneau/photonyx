use anyhow::Result;
use px_cli::ListProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn list_projects(_args: ListProjectArgs, printer: Printer) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
