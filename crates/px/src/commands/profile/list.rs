use anyhow::Result;
use px_cli::ListProfileArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn list_profiles(_args: ListProfileArgs, printer: Printer) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
