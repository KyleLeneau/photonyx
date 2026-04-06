// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::FindBestMasterArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn find_best_master(
    _args: FindBestMasterArgs,
    _printer: Printer,
) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
