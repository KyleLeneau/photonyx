use std::path::{Path, PathBuf};

use anyhow::Result;
use px_cli::StackProjectArgs;
use px_configuration::{
    Framing, GridMosiacFraming, ProjectLinearStack, SingleFraming, SpiralMosiacFraming,
};
use px_conventions::{observation::ObservationPath, project::ProjectPath};
use px_pipeline::{
    master_light::{self, CreateMasterLightPipeline},
    project::{register::RegisterMasterLightPipeline, spiral_mosiac::SpiralMosiacPipeline},
};
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

    match config.framing {
        Framing::Single(single_framing) => {
            stack_single_framing(single_framing, &project.root, printer, args.clean).await?;
        }
        Framing::SpiralMosiac(spiral_framing) => {
            stack_spiral_mosiac_framing(spiral_framing, &project.root, printer, args.clean).await?;
        }
        Framing::GridMosiac(_grid_framing) => {
            stack_grid_mosiac_framing(_grid_framing, &project.root, printer, args.clean).await?;
        }
    }

    Ok(ExitStatus::Success)
}

async fn stack_single_framing(
    framing: SingleFraming,
    project_dir: &Path,
    printer: Printer,
    clean: bool,
) -> Result<()> {
    let ext = FitsExt::FIT;

    let mut master_lights: Vec<PathBuf> = Vec::new();
    for stack in framing.master_lights {
        let builder = Builder::default()
            .output_sink(siril_sys::OutputSink::Discard)
            .use_extension(ext.clone());

        let master_light =
            create_master_light(builder, &stack, project_dir, printer, clean).await?;
        master_lights.push(master_light);
    }

    if master_lights.len() > 1 {
        let builder = Builder::default()
            .output_sink(siril_sys::OutputSink::Inherit)
            .use_extension(ext.clone());
        register_single_framing(builder, master_lights, project_dir, printer).await?;
    }

    printer.success("Project Stacking completed")?;
    Ok(())
}

async fn create_master_light(
    siril_builder: Builder,
    stack: &ProjectLinearStack,
    project_dir: &Path,
    printer: Printer,
    clean: bool,
) -> Result<PathBuf> {
    let ext = siril_builder.clone().ext();

    // TODO: Bail if the master light is already present and clean is not passed
    let existing =
        master_light::master_light_path(project_dir, &stack.name).with_extension(ext.to_string());
    if existing.exists() && !clean {
        printer.warn("master light already exists, use --clean")?;
        return Ok(existing);
    }

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
        .maybe_filter(stack.filter.clone())
        .out_folder(project_dir.to_path_buf())
        .build()
        .run(DefaultPipelineReporter::from(printer))
        .await?;

    // TODO: Save this output file name to the project config?

    // Pretty print the result
    printer.success(format!("Master LIGHT stacking completed: {:?}", master))?;

    Ok(master.path)
}

async fn register_single_framing(
    siril_builder: Builder,
    master_lights: Vec<PathBuf>,
    project_dir: &Path,
    printer: Printer,
) -> Result<()> {
    let ext = siril_builder.clone().ext();
    RegisterMasterLightPipeline::builder()
        .siril_builder(siril_builder)
        .ext(ext)
        .master_lights(master_lights)
        .out_folder(project_dir.to_path_buf())
        .build()
        .run_min_frame(DefaultPipelineReporter::from(printer))
        .await?;

    // TODO: Save this output file name to the project config?

    // Pretty print the result
    printer.success("Master LIGHT registration completed")?;

    Ok(())
}

async fn stack_spiral_mosiac_framing(
    framing: SpiralMosiacFraming,
    project_dir: &Path,
    printer: Printer,
    clean: bool,
) -> Result<PathBuf> {
    let ext = FitsExt::FIT;

    // TODO: Bail if the master light is already present and clean is not passed
    let existing =
        master_light::master_light_path(project_dir, &framing.name).with_extension(ext.to_string());
    if existing.exists() && !clean {
        printer.warn("master light already exists, use --clean")?;
        return Ok(existing);
    }

    let builder = Builder::default()
        .output_sink(siril_sys::OutputSink::Discard)
        .use_extension(ext.clone());

    let light_folders = framing
        .observations
        .iter()
        .map(|o| ObservationPath::single(&o.path).map(|op| op.pp_path().to_path_buf()))
        .collect::<Result<Vec<_>, _>>()?;

    let master = SpiralMosiacPipeline::builder()
        .siril_builder(builder)
        .ext(ext)
        .light_folders(light_folders)
        .name(framing.name.clone())
        .maybe_filter(framing.filter.clone())
        .maybe_feather_pixels(Some(framing.feather_pixels))
        .out_folder(project_dir.to_path_buf())
        .build()
        .run(DefaultPipelineReporter::from(printer))
        .await?;

    // TODO: Save this output file name to the project config?

    // Pretty print the result
    printer.success(format!("Master LIGHT stacking completed: {:?}", master))?;

    printer.success("Project Spiral Stacking completed")?;
    Ok(master.path)
}

async fn stack_grid_mosiac_framing(
    _framing: GridMosiacFraming,
    _project_dir: &Path,
    printer: Printer,
    _clean: bool,
) -> Result<()> {
    let ext = FitsExt::FIT;
    let _builder = Builder::default()
        .output_sink(siril_sys::OutputSink::Discard)
        .use_extension(ext.clone());

    printer.success("Project Stacking completed")?;
    Ok(())
}
