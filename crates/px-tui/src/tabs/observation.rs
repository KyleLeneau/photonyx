use std::path::Path;

use anyhow::Result;
use px_index::{CalibrationRecord, MasterKind, ObservationWithMasters, ProfileIndex};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Cell, Clear, List, ListItem, ListState, Paragraph, Row, TableState},
};

use crate::widgets::{DASH, centered_rect, styled_table};

/// In-progress edit of an observation's calibration links. `kind` selects which of
/// bias/dark/flat is being edited; `list_state` tracks the cursor within that kind's master
/// list, where index `0` is always the synthetic "None (unlink)" entry.
struct CalibrationEditor {
    kind: MasterKind,
    list_state: ListState,
}

/// A calibration-link change staged by the editor, ready for `ProfileIndex::set_calibration_link`
/// or `unlink_calibration`.
pub struct PendingCalibrationLink {
    pub observation_id: String,
    pub kind: MasterKind,
    pub master_id: Option<String>,
}

#[derive(Default)]
pub struct ObservationTab {
    records: Vec<ObservationWithMasters>,
    masters: Vec<CalibrationRecord>,
    state: TableState,
    editor: Option<CalibrationEditor>,
}

impl ObservationTab {
    pub async fn load(&mut self, index: &ProfileIndex) -> Result<()> {
        self.records = index.list_observations_with_masters().await?;
        self.masters = index.list_masters(None).await?;
        if self.records.is_empty() {
            self.state.select(None);
            self.editor = None;
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

    pub fn is_editing(&self) -> bool {
        self.editor.is_some()
    }

    /// Open the calibration-link editor for the currently selected observation, starting on the
    /// bias slot. No-op if nothing is selected.
    pub fn begin_edit(&mut self) {
        if self.state.selected().is_none() {
            return;
        }
        let kind = MasterKind::Bias;
        let mut list_state = ListState::default();
        list_state.select(Some(self.initial_selection(kind)));
        self.editor = Some(CalibrationEditor { kind, list_state });
    }

    pub fn cancel_edit(&mut self) {
        self.editor = None;
    }

    pub fn editor_next_kind(&mut self) {
        self.cycle_kind(next_kind);
    }

    pub fn editor_prev_kind(&mut self) {
        self.cycle_kind(prev_kind);
    }

    fn cycle_kind(&mut self, step: fn(MasterKind) -> MasterKind) {
        let Some(kind) = self.editor.as_ref().map(|e| e.kind) else {
            return;
        };
        let new_kind = step(kind);
        let selection = self.initial_selection(new_kind);
        if let Some(editor) = &mut self.editor {
            editor.kind = new_kind;
            editor.list_state.select(Some(selection));
        }
    }

    pub fn editor_select_next(&mut self) {
        let Some(kind) = self.editor.as_ref().map(|e| e.kind) else {
            return;
        };
        let max = self.masters.iter().filter(|m| m.kind == kind).count();
        if let Some(editor) = &mut self.editor {
            let current = editor.list_state.selected().unwrap_or(0);
            editor.list_state.select(Some((current + 1).min(max)));
        }
    }

    pub fn editor_select_previous(&mut self) {
        if let Some(editor) = &mut self.editor {
            let current = editor.list_state.selected().unwrap_or(0);
            editor.list_state.select(Some(current.saturating_sub(1)));
        }
    }

    /// Jump the cursor straight to the "None (unlink)" entry.
    pub fn editor_select_none(&mut self) {
        if let Some(editor) = &mut self.editor {
            editor.list_state.select(Some(0));
        }
    }

    /// Selection index for `kind` that lands on the observation's currently linked master (or
    /// `0`, the "None" entry, if it has none / isn't linked to a master still in the index).
    fn initial_selection(&self, kind: MasterKind) -> usize {
        let Some(record) = self.state.selected().and_then(|i| self.records.get(i)) else {
            return 0;
        };
        match current_link(record, kind) {
            None => 0,
            Some(path) => self
                .masters
                .iter()
                .filter(|m| m.kind == kind)
                .position(|m| m.master_path == path)
                .map(|i| i + 1)
                .unwrap_or(0),
        }
    }

    /// Consume the editor's current cursor position into a pending DB change. Returns `None` if
    /// no editor is open (nothing to commit).
    pub fn pending_commit(&self) -> Option<PendingCalibrationLink> {
        let editor = self.editor.as_ref()?;
        let record = self.state.selected().and_then(|i| self.records.get(i))?;
        let selected = editor.list_state.selected().unwrap_or(0);
        let master_id = if selected == 0 {
            None
        } else {
            self.masters
                .iter()
                .filter(|m| m.kind == editor.kind)
                .nth(selected - 1)
                .map(|m| m.id.clone())
        };

        Some(PendingCalibrationLink {
            observation_id: record.id.clone(),
            kind: editor.kind,
            master_id,
        })
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

        if self.editor.is_some() {
            self.draw_editor(frame, area);
        }
    }

    fn draw_editor(&mut self, frame: &mut Frame, area: Rect) {
        let Some(record) = self.state.selected().and_then(|i| self.records.get(i)) else {
            self.editor = None;
            return;
        };
        let Some(editor) = &mut self.editor else {
            return;
        };

        let popup = centered_rect(70, 70, area);
        frame.render_widget(Clear, popup);

        let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(0)]).split(popup);

        let kind_line = [MasterKind::Bias, MasterKind::Dark, MasterKind::Flat]
            .into_iter()
            .map(|k| {
                if k == editor.kind {
                    format!("[{}]", kind_label(k))
                } else {
                    format!(" {} ", kind_label(k))
                }
            })
            .collect::<Vec<_>>()
            .join("   ");

        let header = Paragraph::new(kind_line).block(Block::bordered().title(format!(
            " Edit Calibration — {} ({}) ",
            record.target_name, record.date
        )));
        frame.render_widget(header, chunks[0]);

        let linked_path = current_link(record, editor.kind);
        let items: Vec<ListItem> =
            std::iter::once(ListItem::new(none_label(linked_path.is_none())))
                .chain(
                    self.masters
                        .iter()
                        .filter(|m| m.kind == editor.kind)
                        .map(|m| {
                            let is_linked = linked_path == Some(m.master_path.as_str());
                            let marker = if is_linked { "\u{2713} " } else { "  " };
                            ListItem::new(format!("{marker}{}  {}", m.date, m.master_path))
                        }),
                )
                .collect();

        let list =
            List::new(items)
                .block(Block::bordered().title(
                    " Left/Right: kind  Up/Down: select  Enter: link  x: clear  Esc: close ",
                ))
                .highlight_style(Style::default().add_modifier(Modifier::REVERSED))
                .highlight_symbol("> ");

        frame.render_stateful_widget(list, chunks[1], &mut editor.list_state);
    }
}

fn kind_label(kind: MasterKind) -> &'static str {
    match kind {
        MasterKind::Bias => "Bias",
        MasterKind::Dark => "Dark",
        MasterKind::Flat => "Flat",
    }
}

fn next_kind(kind: MasterKind) -> MasterKind {
    match kind {
        MasterKind::Bias => MasterKind::Dark,
        MasterKind::Dark => MasterKind::Flat,
        MasterKind::Flat => MasterKind::Bias,
    }
}

fn prev_kind(kind: MasterKind) -> MasterKind {
    match kind {
        MasterKind::Bias => MasterKind::Flat,
        MasterKind::Dark => MasterKind::Bias,
        MasterKind::Flat => MasterKind::Dark,
    }
}

fn current_link(record: &ObservationWithMasters, kind: MasterKind) -> Option<&str> {
    match kind {
        MasterKind::Bias => record.bias_path.as_deref(),
        MasterKind::Dark => record.dark_path.as_deref(),
        MasterKind::Flat => record.flat_path.as_deref(),
    }
}

fn none_label(active: bool) -> String {
    let marker = if active { "\u{2713} " } else { "  " };
    format!("{marker}None (unlink)")
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
