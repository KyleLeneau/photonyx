// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::InitProfileArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn init_profile(
    _args: InitProfileArgs,
    _printer: Printer,
) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
