// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::ListProfileArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn list_profiles(
    _args: ListProfileArgs,
    _printer: Printer,
) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
