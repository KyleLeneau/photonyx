use anyhow::Result;
use px_cli::InitProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn init_project(_args: InitProjectArgs, printer: Printer) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
