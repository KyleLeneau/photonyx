// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::StackProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn stack_project_observations(
    _args: StackProjectArgs,
    _printer: Printer,
) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
