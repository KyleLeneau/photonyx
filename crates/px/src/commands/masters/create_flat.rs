// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::CreateFlatMasterArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn create_master_flat(
    _args: CreateFlatMasterArgs,
    _printer: Printer,
) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
