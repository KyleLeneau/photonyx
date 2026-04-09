use anyhow::Result;
use px_cli::InitProjectArgs;
use px_configuration::ProjectConfig;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn init_project(args: InitProjectArgs, printer: Printer) -> Result<ExitStatus> {
    let project_dir = &args.path;

    if ProjectConfig::exists(project_dir) {
        printer.error(format!(
            "project already exists at `{}`",
            project_dir.display()
        ))?;
        return Ok(ExitStatus::Failure);
    }

    tokio::fs::create_dir_all(project_dir).await?;

    let name = args.name.unwrap_or_else(|| {
        project_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string()
    });

    let config = ProjectConfig::new(name, args.description);
    config.save(project_dir)?;

    printer.success(format!(
        "initialized project `{}` at `{}`",
        config.name,
        project_dir.display()
    ))?;

    Ok(ExitStatus::Success)
}
