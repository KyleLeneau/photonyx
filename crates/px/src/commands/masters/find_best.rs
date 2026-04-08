use anyhow::Result;
use px_cli::FindBestMasterArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn find_best_master(
    _args: FindBestMasterArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
