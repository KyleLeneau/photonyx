// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::InitProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn init_project(_args: InitProjectArgs, _printer: Printer) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
