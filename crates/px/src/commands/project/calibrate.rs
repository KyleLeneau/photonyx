use anyhow::Result;
use px_cli::CalibrateProjectArgs;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn calibrate_project(
    args: CalibrateProjectArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    // Find the project dir and config to work with
    let (project_dir, config) = match super::find_and_load_project(args.project) {
        Ok(tuple) => tuple,
        Err(e) => {
            printer.error(format!("{e}"))?;
            return Ok(ExitStatus::Failure);
        }
    };

    printer.info(format!(
        "project_dir: {:?}, config: {:?}",
        project_dir.display(),
        config
    ))?;

    Ok(ExitStatus::Success)
}
