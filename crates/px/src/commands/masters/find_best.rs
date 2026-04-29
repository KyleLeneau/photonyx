use anyhow::Result;
use px_cli::FindBestMasterArgs;
use px_index::ProfileIndex;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn find_best_master(
    _args: FindBestMasterArgs,
    printer: Printer,
    _index: ProfileIndex,
) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
