use anyhow::Result;
use px_cli::InitProfileArgs;
use px_conventions::profile::ProfilePath;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn init_profile(args: InitProfileArgs, printer: Printer) -> Result<ExitStatus> {
    let profile_dir = args.path;
    match ProfilePath::new(profile_dir) {
        Ok(path) => {
            printer.success(format!("Initialized profile at `{}`", path.root.display()))?;
            return Ok(ExitStatus::Success);
        }
        Err(e) => {
            printer.error(e.to_string())?;
            return Ok(ExitStatus::Failure);
        }
    }
}
