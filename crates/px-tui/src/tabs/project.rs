use std::path::{Path, PathBuf};

use anyhow::Result;
use px_configuration::{Framing, ProjectConfig, SyncPolicy};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    widgets::{Cell, Row, TableState},
};

use crate::widgets::styled_table;

struct ProjectEntry {
    path: PathBuf,
    config: ProjectConfig,
}

#[derive(Default)]
pub struct ProjectTab {
    projects: Vec<ProjectEntry>,
    state: TableState,
}

/// How many directory levels to descend from the profile root while looking for
/// `px_project.yaml` files. Bounded so a large observation tree doesn't turn a refresh into a
/// full filesystem walk.
const MAX_SCAN_DEPTH: u32 = 6;

impl ProjectTab {
    pub fn load(&mut self, profile_root: &Path) -> Result<()> {
        let mut found = Vec::new();
        scan_dir(profile_root, 0, &mut found)?;
        found.sort_by(|a, b| a.config.name.cmp(&b.config.name));
        self.projects = found;

        if self.projects.is_empty() {
            self.state.select(None);
        } else {
            let selected = self
                .state
                .selected()
                .unwrap_or(0)
                .min(self.projects.len() - 1);
            self.state.select(Some(selected));
        }
        Ok(())
    }

    pub fn select_next(&mut self) {
        if !self.projects.is_empty() {
            self.state.select_next();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.projects.is_empty() {
            self.state.select_previous();
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, profile_root: &Path) {
        let rows: Vec<Row> = self
            .projects
            .iter()
            .map(|p| {
                let target = p.config.target.as_deref().unwrap_or("-").to_string();
                let sync = match p.config.sync_policy {
                    SyncPolicy::Auto => "auto",
                    SyncPolicy::Manual => "manual",
                };
                let path = p
                    .path
                    .strip_prefix(profile_root)
                    .unwrap_or(&p.path)
                    .display()
                    .to_string();

                Row::new([
                    Cell::from(p.config.name.clone()),
                    Cell::from(target),
                    Cell::from(framing_summary(&p.config.framing)),
                    Cell::from(sync),
                    Cell::from(path),
                ])
            })
            .collect();

        let table = styled_table(
            format!("Projects ({})", self.projects.len()),
            vec!["NAME", "TARGET", "FRAMING", "SYNC", "PATH"],
            rows,
            vec![
                Constraint::Length(20),
                Constraint::Length(16),
                Constraint::Length(24),
                Constraint::Length(8),
                Constraint::Min(20),
            ],
        );

        frame.render_stateful_widget(table, area, &mut self.state);
    }
}

fn framing_summary(framing: &Framing) -> String {
    match framing {
        Framing::Single(s) => format!("Single ({} layers)", s.master_lights.len()),
        Framing::SpiralMosiac(s) => format!("Spiral Mosaic ({} obs)", s.observations.len()),
        Framing::GridMosiac(g) => format!("Grid Mosaic ({} panels)", g.master_lights.len()),
    }
}

/// Recursively look for `px_project.yaml` under `dir`. Does not descend into a directory once a
/// project has been found there, since project trees don't nest.
fn scan_dir(dir: &Path, depth: u32, found: &mut Vec<ProjectEntry>) -> Result<()> {
    if depth > MAX_SCAN_DEPTH {
        return Ok(());
    }

    if ProjectConfig::exists(dir) {
        if let Ok(config) = ProjectConfig::load(dir) {
            found.push(ProjectEntry {
                path: dir.to_path_buf(),
                config,
            });
        }
        return Ok(());
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return Ok(()),
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.'))
        {
            continue;
        }
        scan_dir(&path, depth + 1, found)?;
    }

    Ok(())
}
