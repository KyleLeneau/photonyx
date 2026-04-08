// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::ListProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn list_projects(_args: ListProjectArgs, _printer: Printer) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
