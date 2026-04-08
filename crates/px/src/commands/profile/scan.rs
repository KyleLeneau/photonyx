use anyhow::Result;
use px_cli::ScanProfileArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn scan_profile(_args: ScanProfileArgs, printer: Printer) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
