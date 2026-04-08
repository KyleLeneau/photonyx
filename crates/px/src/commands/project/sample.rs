// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::SampleProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn create_project_samples(
    _args: SampleProjectArgs,
    _printer: Printer,
) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
