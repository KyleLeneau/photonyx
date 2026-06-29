use std::{collections::HashSet, path::PathBuf};

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use px_cli::EditProjectArgs;
use px_configuration::{
    Framing, ObservationEntry, ProjectLinearStack, SyncPolicy,
};
use px_conventions::project::ProjectPath;
use px_index::{ObservationRecord, ProfileIndex};
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, List, ListItem, ListState, Paragraph, Wrap,
    },
};

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn edit_project(
    args: EditProjectArgs,
    printer: Printer,
    profile_index: ProfileIndex,
) -> Result<ExitStatus> {
    let project = match ProjectPath::find(args.project) {
        Ok(p) => p,
        Err(e) => {
            printer.error(format!("{e}"))?;
            return Ok(ExitStatus::Failure);
        }
    };

    let mut config = project.load_config()?;

    let target = match &config.target {
        Some(t) => t.clone(),
        None => {
            printer.error(
                "no target is set on this project — cannot open editor.\n\
                 Add `target: <name>` to px_project.yaml or re-run `px project init --target <name>`.",
            )?;
            return Ok(ExitStatus::Failure);
        }
    };

    // Load all observations for this target from the index.
    let all_obs = profile_index
        .list_observations_by_target(Some(&target))
        .await?;

    // Collect all unique filters present in the index for this target.
    let index_filters: Vec<String> = {
        let mut seen = std::collections::BTreeSet::new();
        for o in &all_obs {
            seen.insert(o.filter.clone());
        }
        seen.into_iter().collect()
    };

    // Build the flat list of editable layers from the framing.
    let mut layers = extract_layers(&config.framing);

    // Ensure every index filter has at least a placeholder layer so new ones can be added.
    for filter in &index_filters {
        if !layers.iter().any(|l| l.filter.as_deref() == Some(filter.as_str())) {
            layers.push(EditLayer::new_empty(filter.clone()));
        }
    }

    let mut app = EditApp::new(layers, all_obs, config.sync_policy.clone());
    let result = ratatui::run(|terminal| app.run(terminal))?;

    if result == AppResult::Saved {
        apply_layers_to_framing(&app.layers, &mut config.framing);
        config.save(&project.root)?;
        printer.success("project saved")?;
    } else {
        printer.info("edit cancelled — no changes written")?;
    }

    Ok(ExitStatus::Success)
}

// ---------------------------------------------------------------------------
// Layer model
// ---------------------------------------------------------------------------

/// A flat, editable view of one `ProjectLinearStack` (or a new empty one).
#[derive(Debug, Clone)]
struct EditLayer {
    /// Display name for this layer.
    name: String,
    /// The filter this layer matches against the index.
    filter: Option<String>,
    /// Paths currently assigned to this layer (raw_path from index).
    assigned: HashSet<PathBuf>,
    /// Whether this layer already existed in the project (false = newly created in editor).
    pre_existing: bool,
}

impl EditLayer {
    fn new_empty(filter: String) -> Self {
        Self {
            name: filter.clone(),
            filter: Some(filter),
            assigned: HashSet::new(),
            pre_existing: false,
        }
    }

    fn from_stack(stack: &ProjectLinearStack) -> Self {
        Self {
            name: stack.name.clone(),
            filter: stack.filter.clone(),
            assigned: stack.observations.iter().map(|o| o.path.clone()).collect(),
            pre_existing: true,
        }
    }
}

/// Collect all `ProjectLinearStack` references from any framing variant into a flat list.
fn extract_layers(framing: &Framing) -> Vec<EditLayer> {
    match framing {
        Framing::Single(s) => s.master_lights.iter().map(EditLayer::from_stack).collect(),
        Framing::SpiralMosiac(_) => vec![], // spiral obs are managed differently
        Framing::GridMosiac(g) => g
            .master_lights
            .iter()
            .flat_map(|ml| ml.panels.iter().map(EditLayer::from_stack))
            .collect(),
    }
}

/// Write edited layers back into the framing. New layers are appended to Single framings.
fn apply_layers_to_framing(layers: &[EditLayer], framing: &mut Framing) {
    let make_obs = |layer: &EditLayer| -> Vec<ObservationEntry> {
        let mut paths: Vec<PathBuf> = layer.assigned.iter().cloned().collect();
        paths.sort();
        paths.into_iter().map(|p| ObservationEntry { path: p }).collect()
    };

    match framing {
        Framing::Single(s) => {
            // Update existing stacks by name; append new ones.
            for layer in layers {
                if let Some(stack) = s.master_lights.iter_mut().find(|st| st.name == layer.name) {
                    stack.observations = make_obs(layer);
                } else if !layer.assigned.is_empty() {
                    // New layer created in the editor — only persist if observations were assigned.
                    s.master_lights.push(ProjectLinearStack {
                        profile: s
                            .master_lights
                            .first()
                            .map(|st| st.profile.clone())
                            .unwrap_or_default(),
                        name: layer.name.clone(),
                        panel: None,
                        comments: None,
                        filter: layer.filter.clone(),
                        observations: make_obs(layer),
                        extract_background: false,
                    });
                }
            }
        }
        Framing::GridMosiac(g) => {
            for layer in layers {
                for ml in &mut g.master_lights {
                    if let Some(panel) =
                        ml.panels.iter_mut().find(|p| p.name == layer.name)
                    {
                        panel.observations = make_obs(layer);
                    }
                }
            }
        }
        Framing::SpiralMosiac(_) => {}
    }
}

// ---------------------------------------------------------------------------
// TUI application
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq)]
enum AppResult {
    Saved,
    Cancelled,
}

/// Which panel has keyboard focus.
#[derive(Debug, PartialEq)]
enum Focus {
    Layers,
    Observations,
}

struct EditApp {
    layers: Vec<EditLayer>,
    /// All index observations for this target.
    all_obs: Vec<ObservationRecord>,
    sync_policy: SyncPolicy,

    layer_state: ListState,
    obs_state: ListState,
    focus: Focus,
    result: Option<AppResult>,
}

impl EditApp {
    fn new(layers: Vec<EditLayer>, all_obs: Vec<ObservationRecord>, sync_policy: SyncPolicy) -> Self {
        let mut layer_state = ListState::default();
        if !layers.is_empty() {
            layer_state.select(Some(0));
        }
        Self {
            layers,
            all_obs,
            sync_policy,
            layer_state,
            obs_state: ListState::default(),
            focus: Focus::Layers,
            result: None,
        }
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<AppResult> {
        // Seed obs list for initial layer selection.
        self.refresh_obs_state();

        while self.result.is_none() {
            terminal.draw(|f| self.draw(f))?;
            self.handle_events()?;
        }
        Ok(self.result.take().unwrap())
    }

    fn selected_layer(&self) -> Option<&EditLayer> {
        self.layer_state.selected().and_then(|i| self.layers.get(i))
    }

    fn selected_layer_mut(&mut self) -> Option<&mut EditLayer> {
        self.layer_state.selected().and_then(|i| self.layers.get_mut(i))
    }

    /// Observations from the index that match the currently selected layer's filter.
    fn obs_for_selected_layer(&self) -> Vec<&ObservationRecord> {
        match self.selected_layer() {
            Some(layer) => self
                .all_obs
                .iter()
                .filter(|o| layer.filter.as_deref() == Some(o.filter.as_str()))
                .collect(),
            None => vec![],
        }
    }

    fn refresh_obs_state(&mut self) {
        let obs = self.obs_for_selected_layer();
        if obs.is_empty() {
            self.obs_state.select(None);
        } else if self.obs_state.selected().is_none() {
            self.obs_state.select(Some(0));
        }
    }

    fn toggle_selected_obs(&mut self) {
        let sel_obs_idx = match self.obs_state.selected() {
            Some(i) => i,
            None => return,
        };

        let obs_path = {
            let obs_list = self.obs_for_selected_layer();
            match obs_list.get(sel_obs_idx) {
                Some(o) => PathBuf::from(&o.raw_path),
                None => return,
            }
        };

        if let Some(layer) = self.selected_layer_mut() {
            if layer.assigned.contains(&obs_path) {
                layer.assigned.remove(&obs_path);
            } else {
                layer.assigned.insert(obs_path);
            }
        }
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(2)])
            .split(area);

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
            .split(chunks[0]);

        self.draw_layers_panel(frame, main_chunks[0]);
        self.draw_obs_panel(frame, main_chunks[1]);
        self.draw_status_bar(frame, chunks[1]);
    }

    fn draw_layers_panel(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let focused = self.focus == Focus::Layers;
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let items: Vec<ListItem> = self
            .layers
            .iter()
            .map(|layer| {
                let filter_label = layer.filter.as_deref().unwrap_or("—");
                let count = layer.assigned.len();
                let marker = if !layer.pre_existing { " +" } else { "" };
                let line = Line::from(vec![
                    Span::styled(
                        format!("{}{}", filter_label, marker),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(format!("  ({count} obs)")),
                ]);
                ListItem::new(line)
            })
            .collect();

        let block = Block::default()
            .title(" Layers ")
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style);

        let list = List::new(items)
            .block(block)
            .highlight_style(Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD))
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, area, &mut self.layer_state);
    }

    fn draw_obs_panel(&mut self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let focused = self.focus == Focus::Observations;
        let border_style = if focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        // Collect all display data into owned values first to avoid holding a borrow on
        // `self` while we later need `&mut self.obs_state` for render_stateful_widget.
        let (filter_label, rows): (String, Vec<(bool, String, String)>) = {
            let assigned = self
                .selected_layer()
                .map(|l| l.assigned.clone())
                .unwrap_or_default();

            let filter_label = self
                .selected_layer()
                .and_then(|l| l.filter.clone())
                .unwrap_or_else(|| "—".to_string());

            let rows = self
                .obs_for_selected_layer()
                .into_iter()
                .map(|obs| {
                    let path = PathBuf::from(&obs.raw_path);
                    let checked = assigned.contains(&path);
                    let folder = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(obs.raw_path.as_str())
                        .to_string();
                    (checked, obs.date.clone(), folder)
                })
                .collect();

            (filter_label, rows)
        };

        let items: Vec<ListItem> = rows
            .iter()
            .map(|(checked, date, folder)| {
                let checkbox = if *checked { "[✓]" } else { "[ ]" };
                let style = if *checked {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default()
                };
                let line = Line::from(vec![
                    Span::styled(format!("{checkbox} "), style),
                    Span::styled(format!("{date} "), Style::default().fg(Color::Yellow)),
                    Span::raw(folder.as_str()),
                ]);
                ListItem::new(line)
            })
            .collect();

        let title = format!(" Observations — filter: {filter_label} ");
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style);

        if items.is_empty() {
            let msg = Paragraph::new("No observations in index for this filter.")
                .block(block)
                .wrap(Wrap { trim: true });
            frame.render_widget(msg, area);
        } else {
            let list = List::new(items)
                .block(block)
                .highlight_style(
                    Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD),
                )
                .highlight_symbol("  ");
            frame.render_stateful_widget(list, area, &mut self.obs_state);
        }
    }

    fn draw_status_bar(&self, frame: &mut Frame, area: ratatui::layout::Rect) {
        let policy_note = if self.sync_policy == SyncPolicy::Manual {
            "  [manual sync]"
        } else {
            ""
        };
        let help = Line::from(vec![
            Span::raw(" "),
            Span::styled("Tab", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" switch panel  "),
            Span::styled("↑↓", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" navigate  "),
            Span::styled("Space", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" toggle obs  "),
            Span::styled("s", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" save  "),
            Span::styled("q/Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(format!(" cancel{policy_note}")),
        ]);
        frame.render_widget(Paragraph::new(help), area);
    }

    fn handle_events(&mut self) -> Result<()> {
        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                return Ok(());
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    self.result = Some(AppResult::Cancelled);
                }
                KeyCode::Char('s') => {
                    self.result = Some(AppResult::Saved);
                }
                KeyCode::Tab => {
                    self.focus = match self.focus {
                        Focus::Layers => Focus::Observations,
                        Focus::Observations => Focus::Layers,
                    };
                }
                KeyCode::Up => self.move_selection(-1),
                KeyCode::Down => self.move_selection(1),
                KeyCode::Char(' ') if self.focus == Focus::Observations => {
                    self.toggle_selected_obs();
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn move_selection(&mut self, delta: i32) {
        match self.focus {
            Focus::Layers => {
                let len = self.layers.len();
                if len == 0 {
                    return;
                }
                let cur = self.layer_state.selected().unwrap_or(0) as i32;
                let next = (cur + delta).rem_euclid(len as i32) as usize;
                self.layer_state.select(Some(next));
                // Reset obs selection when layer changes.
                self.obs_state.select(None);
                self.refresh_obs_state();
            }
            Focus::Observations => {
                let obs = self.obs_for_selected_layer();
                let len = obs.len();
                if len == 0 {
                    return;
                }
                let cur = self.obs_state.selected().unwrap_or(0) as i32;
                let next = (cur + delta).rem_euclid(len as i32) as usize;
                self.obs_state.select(Some(next));
            }
        }
    }
}
