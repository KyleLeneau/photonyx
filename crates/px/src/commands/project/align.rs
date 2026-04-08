use anyhow::Result;
use px_cli::AlignProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn align_project(_args: AlignProjectArgs, printer: Printer) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
