// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::AlignProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn align_project(
    _args: AlignProjectArgs,
    _printer: Printer,
) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
