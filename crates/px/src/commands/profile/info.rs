use anyhow::Result;
use px_cli::ShowProfileArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn show_profile(_args: ShowProfileArgs, printer: Printer) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
