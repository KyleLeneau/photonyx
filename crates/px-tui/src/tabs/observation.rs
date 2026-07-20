use std::path::Path;

use anyhow::Result;
use px_index::{ObservationWithMasters, ProfileIndex};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Style},
    widgets::{Cell, Row, TableState},
};

use crate::widgets::{DASH, styled_table};

#[derive(Default)]
pub struct ObservationTab {
    records: Vec<ObservationWithMasters>,
    state: TableState,
}

impl ObservationTab {
    pub async fn load(&mut self, index: &ProfileIndex) -> Result<()> {
        self.records = index.list_observations_with_masters().await?;
        if self.records.is_empty() {
            self.state.select(None);
        } else {
            let selected = self
                .state
                .selected()
                .unwrap_or(0)
                .min(self.records.len() - 1);
            self.state.select(Some(selected));
        }
        Ok(())
    }

    pub fn select_next(&mut self) {
        if !self.records.is_empty() {
            self.state.select_next();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.records.is_empty() {
            self.state.select_previous();
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect, profile_root: &Path) {
        let rows: Vec<Row> = self
            .records
            .iter()
            .map(|r| {
                let exposure = format!("{:.0}s", r.exposure);
                let frames = r
                    .frame_count
                    .map(|f| f.to_string())
                    .unwrap_or_else(|| DASH.to_string());
                let (cal_symbol, cal_color) = if r.calibrated_path.is_some() {
                    ("\u{2713}", Color::Green)
                } else {
                    ("\u{2717}", Color::Red)
                };
                let bias = r.bias_path.as_deref().map(file_stem).unwrap_or(DASH);
                let dark = r.dark_path.as_deref().map(file_stem).unwrap_or(DASH);
                let flat = r.flat_path.as_deref().map(file_stem).unwrap_or(DASH);
                let path = relative_path(&r.raw_path, profile_root);

                Row::new([
                    Cell::from(r.target_name.clone()),
                    Cell::from(r.date.clone()),
                    Cell::from(r.filter.clone()),
                    Cell::from(exposure),
                    Cell::from(frames),
                    Cell::from(cal_symbol).style(Style::default().fg(cal_color)),
                    Cell::from(bias.to_string()),
                    Cell::from(dark.to_string()),
                    Cell::from(flat.to_string()),
                    Cell::from(path),
                ])
            })
            .collect();

        let table = styled_table(
            format!("Observations ({})", self.records.len()),
            vec![
                "TARGET", "DATE", "FILTER", "EXPOSURE", "FRAMES", "CAL", "BIAS", "DARK", "FLAT",
                "PATH",
            ],
            rows,
            vec![
                Constraint::Length(18),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(8),
                Constraint::Length(6),
                Constraint::Length(4),
                Constraint::Length(20),
                Constraint::Length(20),
                Constraint::Length(20),
                Constraint::Min(20),
            ],
        );

        frame.render_stateful_widget(table, area, &mut self.state);
    }
}

fn file_stem(path: &str) -> &str {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}

fn relative_path(raw_path: &str, profile_root: &Path) -> String {
    Path::new(raw_path)
        .strip_prefix(profile_root)
        .ok()
        .and_then(|p| p.to_str())
        .map(str::to_string)
        .unwrap_or_else(|| raw_path.to_string())
}
