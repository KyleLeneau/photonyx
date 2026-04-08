use anyhow::Result;
use px_cli::SampleProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn create_project_samples(
    _args: SampleProjectArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
