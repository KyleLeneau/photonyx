use anyhow::Result;
use px_cli::InitProfileArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn init_profile(_args: InitProfileArgs, printer: Printer) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
