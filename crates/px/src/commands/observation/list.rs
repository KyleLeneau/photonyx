// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::ListObservationArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn list_observations(
    _args: ListObservationArgs,
    _printer: Printer,
) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
