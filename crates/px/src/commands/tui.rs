use anyhow::Result;
use px_index::ProfileIndex;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn run_tui(printer: Printer, index: ProfileIndex) -> Result<ExitStatus> {
    if let Err(e) = px_tui::run(index).await {
        printer.error(format!("{e}"))?;
        return Ok(ExitStatus::Error);
    }
    Ok(ExitStatus::Success)
}
