use anyhow::Result;
use px_cli::ListMasterArgs;
use px_conventions::profile::ProfilePath;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn list_masters(_args: ListMasterArgs, printer: Printer, _profile_path: ProfilePath,) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
