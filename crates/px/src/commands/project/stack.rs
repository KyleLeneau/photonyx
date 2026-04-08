use anyhow::Result;
use px_cli::StackProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn stack_project_observations(
    _args: StackProjectArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    printer.info("WIP, comming soon")?;
    Ok(ExitStatus::Success)
}
