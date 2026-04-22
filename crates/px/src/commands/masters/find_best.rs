use anyhow::Result;
use px_cli::FindBestMasterArgs;
use px_conventions::profile::ProfilePath;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn find_best_master(
    _args: FindBestMasterArgs,
    printer: Printer,
    _profile_path: ProfilePath,
) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
