// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::CreateDarkMasterArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn create_master_dark(
    _args: CreateDarkMasterArgs,
    _printer: Printer,
) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
