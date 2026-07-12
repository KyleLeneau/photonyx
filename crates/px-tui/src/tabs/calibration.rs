use anyhow::Result;
use px_index::{CalibrationRecord, MasterKind, ProfileIndex};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Style},
    widgets::{Cell, Row, TableState},
};

use crate::widgets::{DASH, styled_table};

#[derive(Default)]
pub struct CalibrationTab {
    records: Vec<CalibrationRecord>,
    state: TableState,
}

impl CalibrationTab {
    pub async fn load(&mut self, index: &ProfileIndex) -> Result<()> {
        self.records = index.list_masters(None).await?;
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

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let rows: Vec<Row> = self
            .records
            .iter()
            .map(|r| {
                let kind_color = match r.kind {
                    MasterKind::Bias => Color::Cyan,
                    MasterKind::Dark => Color::Yellow,
                    MasterKind::Flat => Color::Green,
                };
                let filter = r.filter.as_deref().unwrap_or(DASH).to_string();
                let exposure = r
                    .exposure
                    .map(|e| format!("{e}s"))
                    .unwrap_or_else(|| DASH.to_string());
                let temp = r
                    .temperature
                    .map(|t| format!("{t:.1}"))
                    .unwrap_or_else(|| DASH.to_string());
                let gain = r
                    .gain
                    .map(|g| g.to_string())
                    .unwrap_or_else(|| DASH.to_string());
                let offset = r
                    .offset
                    .map(|o| o.to_string())
                    .unwrap_or_else(|| DASH.to_string());
                let binning = r.binning.clone().unwrap_or_else(|| DASH.to_string());

                Row::new([
                    Cell::from(r.kind.as_str()).style(Style::default().fg(kind_color)),
                    Cell::from(r.date.clone()),
                    Cell::from(filter),
                    Cell::from(exposure),
                    Cell::from(temp),
                    Cell::from(gain),
                    Cell::from(offset),
                    Cell::from(binning),
                    Cell::from(r.master_path.clone()),
                ])
            })
            .collect();

        let table = styled_table(
            format!("Calibration Masters ({})", self.records.len()),
            vec![
                "KIND",
                "DATE",
                "FILTER",
                "EXPOSURE",
                "TEMP C",
                "GAIN",
                "OFFSET",
                "BINNING",
                "MASTER PATH",
            ],
            rows,
            vec![
                Constraint::Length(6),
                Constraint::Length(12),
                Constraint::Length(10),
                Constraint::Length(10),
                Constraint::Length(9),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Min(20),
            ],
        );

        frame.render_stateful_widget(table, area, &mut self.state);
    }
}
