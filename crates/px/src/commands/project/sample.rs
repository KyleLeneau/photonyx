use std::path::Path;

use anyhow::Result;
use px_cli::SampleProjectArgs;
use px_configuration::{
    ColorSampleConfig, Framing, FramingLock, GridMosiacFraming, ProjectLock, SampleOutputFormat,
    SingleFraming,
};
use px_conventions::project::ProjectPath;
use px_pipeline::{
    master_light::{master_light_path, registered_master_light_path},
    project::sample::{
        CreateColorSamplePipeline, FilteredStack, SampleOutputFormats, detect_color_mixes,
    },
};
use siril_sys::{Builder, FitsExt, OutputSink};

use crate::{ExitStatus, printer::Printer, reporters::DefaultPipelineReporter};

pub(crate) async fn create_project_samples(
    args: SampleProjectArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    let project = match ProjectPath::find(args.project) {
        Ok(path) => path,
        Err(e) => {
            printer.error(format!("{e}"))?;
            return Ok(ExitStatus::Failure);
        }
    };

    let config = project.load_config()?;
    let sample_config = config.color_sample.unwrap_or_default();
    let ext = FitsExt::FIT;

    // Load lock for precise output paths; falls back to naming conventions when absent.
    let lock = ProjectLock::load(&project.root)?;

    let stacks = match &config.framing {
        Framing::Single(framing) => {
            collect_single_framing(framing, &project.root, &ext, lock.as_ref())
        }
        Framing::GridMosiac(framing) => {
            collect_grid_mosiac_framing(framing, &project.root, &ext, lock.as_ref())
        }
        Framing::SpiralMosiac(_) => {
            printer.warn(
                "px project sample does not support SpiralMosiac framing \
                 (single OSC layer — no multi-filter composition possible)",
            )?;
            return Ok(ExitStatus::Failure);
        }
    };

    let missing: Vec<&FilteredStack> = stacks.iter().filter(|s| !s.path.exists()).collect();
    for m in &missing {
        printer.warn(format!(
            "stack output not found, skipping: {} ({})",
            m.name,
            m.path.display()
        ))?;
    }
    let stacks: Vec<FilteredStack> = stacks.into_iter().filter(|s| s.path.exists()).collect();

    if stacks.is_empty() {
        printer.error(
            "No stack outputs found. Run `px project stack` first to produce master lights.",
        )?;
        return Ok(ExitStatus::Failure);
    }

    let samples = detect_color_mixes(&stacks, &sample_config.exclude_mixes);

    if samples.is_empty() {
        printer.warn("No color mixes could be formed from the available filter stacks.")?;
        let filter_list: Vec<_> = stacks
            .iter()
            .map(|s| s.filter.as_deref().unwrap_or(&s.name))
            .collect();
        printer.info(format!("  Available filters: {}", filter_list.join(", ")))?;
        printer
            .info("  RGB requires R+G+B  |  SHO requires SII+Ha+OIII  |  HOO requires Ha+OIII")?;
        return Ok(ExitStatus::Failure);
    }

    let output_formats = to_pipeline_formats(&sample_config);
    let samples_dir = project.root.join("samples");

    for sample in samples {
        let label = sample.dir_name();
        printer.info(format!("Producing {label} sample..."))?;

        let builder = Builder::default()
            .output_sink(OutputSink::Discard)
            .use_extension(ext.clone());

        let result = CreateColorSamplePipeline::builder()
            .siril_builder(builder)
            .ext(ext.clone())
            .sample(sample)
            .enable_pcc(sample_config.enable_pcc)
            .output_formats(output_formats)
            .out_folder(samples_dir.clone())
            .build()
            .run(DefaultPipelineReporter::from(printer))
            .await;

        match result {
            Ok(out_dir) => {
                printer.success(format!("{label} sample written to: {}", out_dir.display()))?;
            }
            Err(e) => {
                printer.error(format!("{label} sample failed: {e}"))?;
            }
        }
    }

    Ok(ExitStatus::Success)
}

/// Collect `FilteredStack` entries for a `SingleFraming` project.
///
/// When a lock is available, `registered_master_light` is used directly (it is always set,
/// even for single-layer projects where it mirrors the master light). Without a lock the
/// function falls back to the naming-convention helpers.
fn collect_single_framing(
    framing: &SingleFraming,
    project_dir: &Path,
    ext: &FitsExt,
    lock: Option<&ProjectLock>,
) -> Vec<FilteredStack> {
    let lock_single = lock.and_then(|l| {
        if let FramingLock::Single(s) = &l.framing {
            Some(s)
        } else {
            None
        }
    });

    framing
        .master_lights
        .iter()
        .map(|stack| {
            let path = lock_single
                .and_then(|s| s.find_layer(&stack.name))
                .and_then(|l| l.registered_master_light.clone())
                .unwrap_or_else(|| {
                    // Fallback: use naming convention
                    let multi = framing.master_lights.len() > 1;
                    if multi {
                        registered_master_light_path(project_dir, &stack.name)
                    } else {
                        master_light_path(project_dir, &stack.name)
                    }
                    .with_extension(ext.to_string())
                });

            FilteredStack {
                name: stack.name.clone(),
                filter: stack.filter.clone(),
                path,
            }
        })
        .collect()
}

/// Collect `FilteredStack` entries for a `GridMosiac` project.
///
/// Uses lock `registered_master_light` per grid layer when available.
fn collect_grid_mosiac_framing(
    framing: &GridMosiacFraming,
    project_dir: &Path,
    ext: &FitsExt,
    lock: Option<&ProjectLock>,
) -> Vec<FilteredStack> {
    let lock_grid = lock.and_then(|l| {
        if let FramingLock::GridMosiac(g) = &l.framing {
            Some(g)
        } else {
            None
        }
    });

    framing
        .master_lights
        .iter()
        .map(|layer| {
            let path = lock_grid
                .and_then(|g| g.find_layer(&layer.name))
                .and_then(|l| l.registered_master_light.clone())
                .unwrap_or_else(|| {
                    // Fallback: use naming convention
                    let multi = framing.master_lights.len() > 1;
                    if multi {
                        registered_master_light_path(project_dir, &layer.name)
                    } else {
                        master_light_path(project_dir, &layer.name)
                    }
                    .with_extension(ext.to_string())
                });

            FilteredStack {
                name: layer.name.clone(),
                filter: layer.filter.clone(),
                path,
            }
        })
        .collect()
}

fn to_pipeline_formats(config: &ColorSampleConfig) -> SampleOutputFormats {
    let mut formats = SampleOutputFormats::default();
    for fmt in &config.output_formats {
        match fmt {
            SampleOutputFormat::Fit => formats.fit = true,
            SampleOutputFormat::Tiff => formats.tiff = true,
            SampleOutputFormat::Png => formats.png = true,
            SampleOutputFormat::Jpg => formats.jpg = true,
        }
    }
    formats
}
