// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::CalibrateProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn calibrate_project(
    _args: CalibrateProjectArgs,
    _printer: Printer,
) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
