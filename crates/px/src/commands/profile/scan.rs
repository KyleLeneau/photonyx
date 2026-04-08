// use std::fmt::Write;

use anyhow::Result;
// use owo_colors::OwoColorize;
use px_cli::ScanProfileArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn scan_profile(_args: ScanProfileArgs, _printer: Printer) -> Result<ExitStatus> {
    Ok(ExitStatus::Success)
}
