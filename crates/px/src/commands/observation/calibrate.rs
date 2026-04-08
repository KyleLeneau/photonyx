// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::CalibrateObservationArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn calibrate_observation(
    _args: CalibrateObservationArgs,
    _printer: Printer,
) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
