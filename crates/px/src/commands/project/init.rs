use anyhow::Result;
use inquire::{CustomType, Select, Text};
use px_cli::{FramingType, InitProjectArgs};
use px_configuration::{
    Framing, GridMosiacFraming, GridMosiacMasterLight, ObservationEntry, ProjectConfig,
    ProjectLinearStack, SingleFraming, SpiralMosiacFraming,
};
use px_index::ProfileIndex;
use std::path::PathBuf;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn init_project(
    args: InitProjectArgs,
    printer: Printer,
    profile_index: ProfileIndex,
) -> Result<ExitStatus> {
    let profile_root = profile_index.profile.root.clone();

    // Destructure so every field is independently owned.
    let InitProjectArgs {
        path: arg_path,
        name: arg_name,
        description: arg_description,
        framing: arg_framing,
        stack_name,
        filter,
        feather_pixels,
        no_interactive,
    } = args;

    // --- name ---
    let name = if let Some(n) = arg_name {
        n
    } else if no_interactive {
        match &arg_path {
            Some(p) => p
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unnamed")
                .to_string(),
            None => {
                printer
                    .error("--name is required in non-interactive mode when --path is omitted")?;
                return Ok(ExitStatus::Failure);
            }
        }
    } else {
        let default = arg_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("my_project")
            .to_string();
        Text::new("Project name:").with_default(&default).prompt()?
    };

    // --- path ---
    let project_dir = {
        let slug = name.replace(' ', "_");
        arg_path.unwrap_or_else(|| profile_root.join("PROJECTS").join(slug))
    };

    if ProjectConfig::exists(&project_dir) {
        printer.error(format!(
            "project already exists at `{}`",
            project_dir.display()
        ))?;
        return Ok(ExitStatus::Failure);
    }

    // --- description ---
    let description = if let Some(d) = arg_description {
        Some(d)
    } else if no_interactive {
        None
    } else {
        let input = Text::new("Description (optional):")
            .with_default("")
            .prompt()?;
        if input.is_empty() { None } else { Some(input) }
    };

    // --- framing type ---
    let framing_type = if let Some(f) = arg_framing {
        f
    } else if no_interactive {
        FramingType::Single
    } else {
        let options = vec!["single", "spiral-mosiac", "grid-mosiac"];
        let choice = Select::new("Framing type:", options).prompt()?;
        match choice {
            "spiral-mosiac" => FramingType::SpiralMosiac,
            "grid-mosiac" => FramingType::GridMosiac,
            _ => FramingType::Single,
        }
    };

    // --- build framing config ---
    let framing = match framing_type {
        FramingType::Single => build_single_framing(
            stack_name.as_deref(),
            filter.as_deref(),
            no_interactive,
            &profile_root,
        )?,
        FramingType::SpiralMosiac => build_spiral_framing(
            stack_name.as_deref(),
            filter.as_deref(),
            feather_pixels,
            no_interactive,
        )?,
        FramingType::GridMosiac => build_grid_framing(
            stack_name.as_deref(),
            filter.as_deref(),
            no_interactive,
            &profile_root,
        )?,
    };

    tokio::fs::create_dir_all(&project_dir).await?;

    let config = ProjectConfig {
        name: name.clone(),
        description,
        framing,
    };
    config.save(&project_dir)?;

    printer.success(format!(
        "initialized project `{}` at `{}`",
        config.name,
        project_dir.display()
    ))?;

    Ok(ExitStatus::Success)
}

fn build_single_framing(
    stack_name: Option<&str>,
    filter: Option<&str>,
    no_interactive: bool,
    profile_root: &std::path::Path,
) -> Result<Framing> {
    let example_obs = PathBuf::from("observations").join("session_2025_01_01");

    let master_lights = if no_interactive {
        // Non-interactive: single entry from flags
        let name = stack_name.unwrap_or("L_300").to_string();
        let filter = filter.filter(|f| !f.is_empty()).map(str::to_string);
        vec![ProjectLinearStack {
            profile: profile_root.to_path_buf(),
            name,
            panel: None,
            comments: Some("edit this entry and add more layers as needed".to_string()),
            filter,
            observations: vec![ObservationEntry { path: example_obs }],
            extract_background: false,
        }]
    } else {
        let count = CustomType::<u32>::new("How many master light layers?")
            .with_default(1)
            .prompt()?;

        let mut layers = Vec::with_capacity(count as usize);
        for i in 1..=count {
            let default_name = stack_name
                .map(|s| {
                    if count == 1 {
                        s.to_string()
                    } else {
                        format!("{s}_{i}")
                    }
                })
                .unwrap_or_else(|| {
                    if count == 1 {
                        "L_30".to_string()
                    } else {
                        format!("L_30_{i}")
                    }
                });

            let name = Text::new(&format!("Layer {i} name:"))
                .with_default(&default_name)
                .prompt()?;

            let default_filter = filter.unwrap_or("").to_string();
            let filter_input = Text::new(&format!(
                "Layer {i} filter (e.g. Ha, LRGB — leave blank to skip):"
            ))
            .with_default(&default_filter)
            .prompt()?;
            let layer_filter = if filter_input.is_empty() {
                None
            } else {
                Some(filter_input)
            };

            layers.push(ProjectLinearStack {
                profile: profile_root.to_path_buf(),
                name,
                panel: None,
                comments: Some("edit observations and profile path as needed".to_string()),
                filter: layer_filter,
                observations: vec![ObservationEntry {
                    path: example_obs.clone(),
                }],
                extract_background: false,
            });
        }
        layers
    };

    Ok(Framing::Single(SingleFraming { master_lights }))
}

fn build_spiral_framing(
    stack_name: Option<&str>,
    filter: Option<&str>,
    feather_pixels: Option<f32>,
    no_interactive: bool,
) -> Result<Framing> {
    let mosaic_name =
        resolve_optional_str(stack_name, no_interactive, "Mosaic name:", "my_mosaic")?;

    let filter = resolve_optional_str(
        filter,
        no_interactive,
        "Filter (e.g. Ha, OSC — leave blank to skip):",
        "",
    )?;
    let filter = if filter.is_empty() {
        None
    } else {
        Some(filter)
    };

    let feather_pixels = feather_pixels.unwrap_or(0.0);

    let example_obs = PathBuf::from("observations").join("session_2025_01_01");

    Ok(Framing::SpiralMosiac(SpiralMosiacFraming {
        name: mosaic_name,
        feather_pixels,
        filter,
        observations: vec![ObservationEntry { path: example_obs }],
    }))
}

fn build_grid_framing(
    stack_name: Option<&str>,
    filter: Option<&str>,
    no_interactive: bool,
    profile_root: &std::path::Path,
) -> Result<Framing> {
    let mosaic_name =
        resolve_optional_str(stack_name, no_interactive, "Mosaic name:", "my_mosaic")?;

    let (rows, cols, filters) = if no_interactive {
        let filters = filter
            .filter(|f| !f.is_empty())
            .map(|f| vec![f.to_string()])
            .unwrap_or_else(|| vec!["OSC".to_string()]);
        (2u32, 2u32, filters)
    } else {
        let cols = CustomType::<u32>::new("How many panels wide?")
            .with_default(2)
            .prompt()?;
        let rows = CustomType::<u32>::new("How many panels tall?")
            .with_default(2)
            .prompt()?;

        let filter_count = CustomType::<u32>::new("How many filters?")
            .with_default(1)
            .prompt()?;

        let mut filters = Vec::with_capacity(filter_count as usize);
        for i in 1..=filter_count {
            let default = if i == 1 { filter.unwrap_or("OSC") } else { "" };
            let f = Text::new(&format!("Filter {i} name:"))
                .with_default(default)
                .prompt()?;
            filters.push(f);
        }
        (rows, cols, filters)
    };

    let example_obs = PathBuf::from("observations").join("session_2025_01_01");
    let multi_filter = filters.len() > 1;

    let mut master_lights = Vec::new();
    for f in filters {
        let filter_val = if f.is_empty() { None } else { Some(f.clone()) };
        let mut panels = Vec::new();

        for row in 1..=rows {
            for col in 1..=cols {
                let panel_id = format!("{row}-{col}");
                let name = if multi_filter {
                    format!("{panel_id}_{f}")
                } else {
                    panel_id.clone()
                };

                panels.push(ProjectLinearStack {
                    profile: profile_root.to_path_buf(),
                    name,
                    panel: Some(panel_id.clone()),
                    comments: Some("edit observations and profile path as needed".to_string()),
                    filter: filter_val.clone(),
                    observations: vec![ObservationEntry {
                        path: example_obs.clone(),
                    }],
                    extract_background: false,
                });
            }
        }

        master_lights.push(GridMosiacMasterLight {
            name: mosaic_name.clone(),
            filter: filter_val.clone(),
            panels,
        });
    }

    Ok(Framing::GridMosiac(GridMosiacFraming { master_lights }))
}

/// Prompt the user for an optional string value, or return the provided value / default directly.
fn resolve_optional_str(
    provided: Option<&str>,
    no_interactive: bool,
    prompt: &str,
    default: &str,
) -> Result<String> {
    if let Some(v) = provided {
        return Ok(v.to_string());
    }
    if no_interactive {
        return Ok(default.to_string());
    }
    Ok(Text::new(prompt).with_default(default).prompt()?)
}
