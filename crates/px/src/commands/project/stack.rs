use std::path::{Path, PathBuf};

use anyhow::Result;
use chrono::Utc;
use px_cli::StackProjectArgs;
use px_configuration::{
    Framing, FramingLock, GridLayerLock, GridMosiacFraming, GridMosiacLock, LayerLock, PanelLock,
    ProjectLinearStack, ProjectLock, SingleFraming, SingleFramingLock, SpiralMosiacFraming,
    SpiralMosiacLock, hash_linear_stack, hash_spiral_framing,
};
use px_conventions::{observation::ObservationPath, project::ProjectPath};
use px_pipeline::{
    master_light::{CreateMasterLightPipeline, registered_master_light_path},
    project::{
        grid_mosiac::GridMosiacPipeline, register::RegisterMasterLightPipeline,
        spiral_mosiac::SpiralMosiacPipeline,
    },
};
use siril_sys::{Builder, FitsExt};

use crate::{ExitStatus, printer::Printer, reporters::DefaultPipelineReporter};

pub(crate) async fn stack_project_observations(
    args: StackProjectArgs,
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
    printer.info(format!(
        "project_dir: {}, framing: {}\n",
        project.root.display(),
        config.framing
    ))?;

    match config.framing {
        Framing::Single(single_framing) => {
            stack_single_framing(single_framing, &project.root, printer, args.clean).await?;
        }
        Framing::SpiralMosiac(spiral_framing) => {
            stack_spiral_mosiac_framing(spiral_framing, &project.root, printer, args.clean).await?;
        }
        Framing::GridMosiac(grid_framing) => {
            stack_grid_mosiac_framing(grid_framing, &project.root, printer, args.clean).await?;
        }
    }

    Ok(ExitStatus::Success)
}

/// Stack a single framing target project (same ra/dec).
/// Uses minimum framing for final cross-layer registration.
async fn stack_single_framing(
    framing: SingleFraming,
    project_dir: &Path,
    printer: Printer,
    clean: bool,
) -> Result<()> {
    let ext = FitsExt::FIT;

    // Load existing lock for dirty-checking; ignore it when --clean.
    let existing_lock = if clean {
        None
    } else {
        ProjectLock::load(project_dir)?
    };

    let old_single = existing_lock.as_ref().and_then(|l| {
        if let FramingLock::Single(s) = &l.framing {
            Some(s)
        } else {
            None
        }
    });

    let mut new_single = SingleFramingLock::default();
    let mut master_light_paths: Vec<PathBuf> = Vec::new();
    let mut any_restacked = false;

    for stack in &framing.master_lights {
        let hash = hash_linear_stack(stack);
        let old_layer = old_single.and_then(|s| s.find_layer(&stack.name));
        let is_dirty = old_layer.is_none_or(|l| l.is_dirty(&hash));

        let ml_path = if !is_dirty {
            let cached = old_layer.unwrap().master_light.clone().unwrap();
            printer.info(format!("layer `{}`: up to date, skipping", stack.name))?;
            cached
        } else {
            any_restacked = true;
            let builder = Builder::default()
                .output_sink(siril_sys::OutputSink::Discard)
                .use_extension(ext.clone());
            printer.info(format!("stacking single framing layer: {:?}", stack.name))?;
            run_master_light(builder, stack, project_dir, printer).await?
        };

        new_single.master_lights.push(LayerLock {
            name: stack.name.clone(),
            input_hash: hash,
            master_light: Some(ml_path.clone()),
            registered_master_light: None, // filled after registration decision
            stacked_at: Some(now_utc()),
        });
        master_light_paths.push(ml_path);

        // Save partial progress after each layer.
        save_lock(project_dir, FramingLock::Single(new_single.clone()))?;
    }

    // Registration: needed when >1 layer; also re-runs if any layer was restacked
    // or if any registered peer is missing from the old lock.
    let needs_reg = master_light_paths.len() > 1;

    if needs_reg {
        let reg_dirty = any_restacked
            || old_single.is_none_or(|s| s.master_lights.iter().any(|l| l.is_registration_dirty()));

        if reg_dirty {
            let builder = Builder::default()
                .output_sink(siril_sys::OutputSink::Discard)
                .use_extension(ext.clone());
            register_layers(builder, master_light_paths, project_dir, printer).await?;

            for (entry, stack) in new_single
                .master_lights
                .iter_mut()
                .zip(&framing.master_lights)
            {
                entry.registered_master_light = Some(
                    registered_master_light_path(project_dir, &stack.name)
                        .with_extension(ext.to_string()),
                );
            }
        } else {
            printer.info("registration: up to date, skipping")?;
            for (entry, stack) in new_single
                .master_lights
                .iter_mut()
                .zip(&framing.master_lights)
            {
                let reg_path = old_single
                    .and_then(|s| s.find_layer(&stack.name))
                    .and_then(|l| l.registered_master_light.clone());
                entry.registered_master_light = reg_path;
            }
        }
    } else {
        // Single layer: registered_master_light mirrors master_light.
        if let Some(entry) = new_single.master_lights.first_mut() {
            entry.registered_master_light = entry.master_light.clone();
        }
    }

    save_lock(project_dir, FramingLock::Single(new_single))?;
    printer.success("Project Stacking completed")?;
    Ok(())
}

/// Stack a spiral mosaic framing.
async fn stack_spiral_mosiac_framing(
    framing: SpiralMosiacFraming,
    project_dir: &Path,
    printer: Printer,
    clean: bool,
) -> Result<()> {
    let ext = FitsExt::FIT;

    let existing_lock = if clean {
        None
    } else {
        ProjectLock::load(project_dir)?
    };

    let old_spiral = existing_lock.as_ref().and_then(|l| {
        if let FramingLock::SpiralMosiac(s) = &l.framing {
            Some(s)
        } else {
            None
        }
    });

    let hash = hash_spiral_framing(&framing);
    let is_dirty = old_spiral.is_none_or(|s| s.is_dirty(&hash));

    let ml_path = if !is_dirty {
        let cached = old_spiral.unwrap().master_light.clone().unwrap();
        printer.info("spiral mosaic: up to date, skipping")?;
        cached
    } else {
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
            .ext(ext.clone())
            .light_folders(light_folders)
            .name(framing.name.clone())
            .maybe_filter(framing.filter.clone())
            .maybe_feather_pixels(Some(framing.feather_pixels))
            .out_folder(project_dir.to_path_buf())
            .build()
            .run(DefaultPipelineReporter::from(printer))
            .await?;

        printer.success(format!(
            "Master LIGHT stacking completed: {:?}",
            master.path
        ))?;
        master.path
    };

    save_lock(
        project_dir,
        FramingLock::SpiralMosiac(SpiralMosiacLock {
            name: framing.name.clone(),
            input_hash: hash,
            master_light: Some(ml_path),
            stacked_at: Some(now_utc()),
        }),
    )?;

    printer.success("Project Spiral Stacking completed")?;
    Ok(())
}

/// Stack a grid mosaic with multiple panels per layer and a final cross-layer registration.
async fn stack_grid_mosiac_framing(
    framing: GridMosiacFraming,
    project_dir: &Path,
    printer: Printer,
    clean: bool,
) -> Result<()> {
    let ext = FitsExt::FIT;
    let builder = Builder::default()
        .output_sink(siril_sys::OutputSink::Discard)
        .use_extension(ext.clone());

    let existing_lock = if clean {
        None
    } else {
        ProjectLock::load(project_dir)?
    };

    let old_grid = existing_lock.as_ref().and_then(|l| {
        if let FramingLock::GridMosiac(g) = &l.framing {
            Some(g)
        } else {
            None
        }
    });

    let mut new_grid = GridMosiacLock::default();
    let mut layer_paths: Vec<PathBuf> = Vec::new();
    let mut any_layer_restacked = false;

    for grid_layer in &framing.master_lights {
        let old_grid_layer = old_grid.and_then(|g| g.find_layer(&grid_layer.name));

        // Stack each panel.
        let mut panel_locks: Vec<PanelLock> = Vec::new();
        let mut panel_paths: Vec<PathBuf> = Vec::new();
        let mut any_panel_restacked = false;

        for panel in &grid_layer.panels {
            let hash = hash_linear_stack(panel);
            let old_panel = old_grid_layer.and_then(|l| l.find_panel(&panel.name));
            let is_dirty = old_panel.is_none_or(|p| p.is_dirty(&hash));

            let ml_path = if !is_dirty {
                let cached = old_panel.unwrap().master_light.clone().unwrap();
                printer.info(format!("panel `{}`: up to date, skipping", panel.name))?;
                cached
            } else {
                any_panel_restacked = true;
                printer.info(format!("stacking grid panel: {:?}", panel.name))?;
                run_master_light(builder.clone(), panel, project_dir, printer).await?
            };

            panel_locks.push(PanelLock {
                name: panel.name.clone(),
                input_hash: hash,
                master_light: Some(ml_path.clone()),
                stacked_at: Some(now_utc()),
            });
            panel_paths.push(ml_path);
        }

        // Stitch panels into a grid master light for this layer.
        let grid_dirty = any_panel_restacked || old_grid_layer.is_none_or(|l| l.is_grid_dirty());

        let grid_ml_path = if !grid_dirty {
            let cached = old_grid_layer.unwrap().master_light.clone().unwrap();
            printer.info(format!(
                "grid layer `{}`: up to date, skipping",
                grid_layer.name
            ))?;
            cached
        } else {
            any_layer_restacked = true;
            let grid_master = GridMosiacPipeline::builder()
                .siril_builder(builder.clone())
                .ext(ext.clone())
                .tile_master_lights(panel_paths)
                .name(grid_layer.name.clone())
                .maybe_filter(grid_layer.filter.clone())
                .background_extract(true)
                .out_folder(project_dir.to_path_buf())
                .build()
                .run(DefaultPipelineReporter::from(printer))
                .await?;

            printer.success(format!(
                "Master LIGHT stacking completed: {:?}",
                grid_master.path
            ))?;
            grid_master.path
        };

        new_grid.master_lights.push(GridLayerLock {
            name: grid_layer.name.clone(),
            panels: panel_locks,
            master_light: Some(grid_ml_path.clone()),
            registered_master_light: None, // filled after registration
            stacked_at: Some(now_utc()),
        });
        layer_paths.push(grid_ml_path);

        // Save partial progress after each grid layer.
        save_lock(project_dir, FramingLock::GridMosiac(new_grid.clone()))?;
    }

    // Final cross-layer registration.
    let needs_reg = layer_paths.len() > 1;

    if needs_reg {
        let reg_dirty = any_layer_restacked
            || old_grid.is_none_or(|g| g.master_lights.iter().any(|l| l.is_registration_dirty()));

        if reg_dirty {
            register_layers(builder, layer_paths, project_dir, printer).await?;

            for (entry, config_layer) in new_grid
                .master_lights
                .iter_mut()
                .zip(&framing.master_lights)
            {
                entry.registered_master_light = Some(
                    registered_master_light_path(project_dir, &config_layer.name)
                        .with_extension(ext.to_string()),
                );
            }
        } else {
            printer.info("registration: up to date, skipping")?;
            for (entry, config_layer) in new_grid
                .master_lights
                .iter_mut()
                .zip(&framing.master_lights)
            {
                let reg_path = old_grid
                    .and_then(|g| g.find_layer(&config_layer.name))
                    .and_then(|l| l.registered_master_light.clone());
                entry.registered_master_light = reg_path;
            }
        }
    } else {
        // Single grid layer: registered_master_light mirrors master_light.
        if let Some(entry) = new_grid.master_lights.first_mut() {
            entry.registered_master_light = entry.master_light.clone();
        }
    }

    save_lock(project_dir, FramingLock::GridMosiac(new_grid))?;
    printer.success("Project Stacking completed")?;
    Ok(())
}

/// Run `CreateMasterLightPipeline` for a single `ProjectLinearStack`.
async fn run_master_light(
    siril_builder: Builder,
    stack: &ProjectLinearStack,
    project_dir: &Path,
    printer: Printer,
) -> Result<PathBuf> {
    let ext = siril_builder.clone().ext();

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
        .background_extract(stack.extract_background)
        .out_folder(project_dir.to_path_buf())
        .build()
        .run(DefaultPipelineReporter::from(printer))
        .await?;

    printer.success(format!(
        "Master LIGHT stacking completed: {:?}\n",
        master.path
    ))?;

    Ok(master.path)
}

/// Register a list of master lights to their minimum frame overlap.
async fn register_layers(
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

    printer.success("Master LIGHT registration completed")?;
    Ok(())
}

fn save_lock(project_dir: &Path, framing: FramingLock) -> Result<()> {
    ProjectLock {
        schema_version: 1,
        framing,
    }
    .save(project_dir)?;
    Ok(())
}

fn now_utc() -> String {
    Utc::now().to_rfc3339()
}
