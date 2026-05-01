use std::path::Path;

use anyhow::Result;
use px_cli::StackProjectArgs;
use px_configuration::ProjectLinearStack;
use px_conventions::{observation::ObservationPath, project::ProjectPath};
use px_pipeline::master_light::CreateMasterLightPipeline;
use siril_sys::{Builder, FitsExt};

use crate::{ExitStatus, printer::Printer, reporters::DefaultPipelineReporter};

pub(crate) async fn stack_project_observations(
    args: StackProjectArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    // Find the project dir and config to work with
    let project = match ProjectPath::find(args.project) {
        Ok(path) => path,
        Err(e) => {
            printer.error(format!("{e}"))?;
            return Ok(ExitStatus::Failure);
        }
    };

    let config = project.load_config()?;
    printer.info(format!(
        "project_dir: {:?}, config: {:?}",
        project.root.display(),
        config
    ))?;

    for stack in config.linear_stacks {
        let ext = FitsExt::FIT;
        let builder = Builder::default()
            .output_sink(siril_sys::OutputSink::Discard)
            .use_extension(ext.clone());

        stack_linear(builder, &stack, ext, &project.root, printer).await?;
        // utils::wait_for_confirm(printer).await;
    }

    Ok(ExitStatus::Success)
}

async fn stack_linear(
    siril_builder: Builder,
    stack: &ProjectLinearStack,
    ext: FitsExt,
    project_dir: &Path,
    printer: Printer,
) -> Result<()> {
    let light_folders = stack
        .observations
        .iter()
        .map(|o| ObservationPath::single(&o.path).map(|op| op.pp_path().to_path_buf()))
        .collect::<Result<Vec<_>, _>>()?;

    let master = CreateMasterLightPipeline::builder()
        .siril_builder(siril_builder)
        .ext(ext)
        .light_folders(light_folders)
        .name(stack.name.clone())
        .out_folder(project_dir.to_path_buf())
        .build()
        .run(DefaultPipelineReporter::from(printer))
        .await?;

    // Pretty print the result
    printer.success(format!("Master LIGHT stacking completed: {:?}", master))?;

    Ok(())
}
