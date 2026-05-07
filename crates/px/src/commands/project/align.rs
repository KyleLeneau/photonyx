// use anyhow::Result;
// use px_cli::AlignProjectArgs;
// use px_conventions::project::ProjectPath;

// use crate::{ExitStatus, printer::Printer};

// pub(crate) async fn align_project(args: AlignProjectArgs, printer: Printer) -> Result<ExitStatus> {
//     // Find the project dir and config to work with
//     let project = match ProjectPath::find(args.project) {
//         Ok(path) => path,
//         Err(e) => {
//             printer.error(format!("{e}"))?;
//             return Ok(ExitStatus::Failure);
//         }
//     };

//     printer.info(format!(
//         "project_dir: {:?}, config: {:?}",
//         project.root.display(),
//         project.load_config()?
//     ))?;

//     Ok(ExitStatus::Success)
// }
