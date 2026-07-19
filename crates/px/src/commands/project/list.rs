use std::fmt::Write as _;
use std::io;

use anyhow::Result;
use px_cli::{ListProjectArgs, OutputFormat};
use px_configuration::{Framing, ProjectConfig, SyncPolicy};
use px_index::ProfileIndex;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Constraint,
    style::{Modifier, Style},
    widgets::{Block, Cell, Padding, Row, Table},
};
use serde::Serialize;

use crate::{ExitStatus, printer::Printer};

#[derive(Debug, Serialize)]
struct ProjectRow {
    name: String,
    description: Option<String>,
    target: Option<String>,
    framing: String,
    layers: usize,
    sync_policy: &'static str,
    path: String,
}

pub(crate) async fn list_projects(
    args: ListProjectArgs,
    printer: Printer,
    profile_index: ProfileIndex,
) -> Result<ExitStatus> {
    let mut rows = Vec::new();

    let mut entries = tokio::fs::read_dir(&profile_index.profile.projects).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_dir() || !ProjectConfig::exists(&path) {
            continue;
        }

        let config = ProjectConfig::load(&path)?;
        rows.push(ProjectRow {
            name: config.name,
            description: config.description,
            target: config.target,
            framing: config.framing.kind_str().to_string(),
            layers: layer_count(&config.framing),
            sync_policy: match config.sync_policy {
                SyncPolicy::Auto => "auto",
                SyncPolicy::Manual => "manual",
            },
            path: path.display().to_string(),
        });
    }

    rows.sort_by(|a, b| a.name.cmp(&b.name));

    if rows.is_empty() {
        printer.info("no projects found in this profile")?;
        return Ok(ExitStatus::Success);
    }

    match args.output {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rows)?;
            writeln!(printer.stdout(), "{json}")?;
        }
        OutputFormat::Pretty => {
            render_table(&rows)?;
        }
    }

    Ok(ExitStatus::Success)
}

fn layer_count(framing: &Framing) -> usize {
    match framing {
        Framing::Single(single) => single.master_lights.len(),
        Framing::SpiralMosiac(_) => 1,
        Framing::GridMosiac(grid) => grid.master_lights.len(),
    }
}

fn render_table(rows: &[ProjectRow]) -> Result<()> {
    let table_rows: Vec<Row> = rows
        .iter()
        .map(|row| {
            Row::new([
                Cell::from(row.name.clone()),
                Cell::from(row.target.clone().unwrap_or_else(|| "-".to_string())),
                Cell::from(row.framing.clone()),
                Cell::from(row.layers.to_string()),
                Cell::from(row.sync_policy),
                Cell::from(row.path.clone()),
            ])
        })
        .collect();

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::with_options(
        backend,
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline((rows.len() + 4) as u16),
        },
    )?;

    terminal.draw(|frame| {
        let table = Table::new(
            table_rows.clone(),
            [
                Constraint::Length(20),
                Constraint::Length(16),
                Constraint::Length(14),
                Constraint::Length(7),
                Constraint::Length(9),
                Constraint::Min(20),
            ],
        )
        .block(
            Block::bordered()
                .title(" Projects ")
                .padding(Padding::horizontal(1)),
        )
        .header(
            Row::new(["NAME", "TARGET", "FRAMING", "LAYERS", "SYNC", "PATH"]).style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        );
        frame.render_widget(table, frame.area());
    })?;

    Ok(())
}
