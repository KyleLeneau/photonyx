use std::future::Future;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use px_index::ProfileIndex;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Paragraph, Tabs},
};

use crate::tabs::{CalibrationTab, ObservationTab, ProfileTab, ProjectTab};

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Calibration,
    Observation,
    Project,
    Profile,
}

impl ActiveTab {
    const ALL: [ActiveTab; 4] = [
        ActiveTab::Calibration,
        ActiveTab::Observation,
        ActiveTab::Project,
        ActiveTab::Profile,
    ];
    const TITLES: [&'static str; 4] = ["Calibration", "Observation", "Project", "Profile"];

    fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap()
    }

    fn next(self) -> Self {
        Self::ALL[(self.index() + 1) % Self::ALL.len()]
    }

    fn previous(self) -> Self {
        Self::ALL[(self.index() + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// Runs an async future to completion from inside the (already async) `run` call. Safe because
/// the surrounding runtime is always the multi-thread flavor set up by `#[tokio::main]`.
fn block_on<F: Future>(fut: F) -> F::Output {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(fut))
}

pub struct App {
    index: ProfileIndex,
    active: ActiveTab,
    calibration: CalibrationTab,
    observation: ObservationTab,
    project: ProjectTab,
    profile: ProfileTab,
    status: Option<String>,
    exit: bool,
}

impl App {
    pub fn new(index: ProfileIndex) -> Self {
        Self {
            index,
            active: ActiveTab::Calibration,
            calibration: CalibrationTab::default(),
            observation: ObservationTab::default(),
            project: ProjectTab::default(),
            profile: ProfileTab::default(),
            status: None,
            exit: false,
        }
    }

    pub async fn load_all(&mut self) -> Result<()> {
        self.calibration.load(&self.index).await?;
        self.observation.load(&self.index).await?;
        self.project.load(&self.index.profile.root)?;
        self.profile.load(&self.index.config);
        Ok(())
    }

    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> Result<()> {
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            if event::poll(Duration::from_millis(200))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.handle_key(key.code)?;
            }

            if self.exit {
                return Ok(());
            }
        }
    }

    fn handle_key(&mut self, code: KeyCode) -> Result<()> {
        // While editing a profile field, every key but Enter/Esc/Backspace is literal input.
        if self.active == ActiveTab::Profile && self.profile.is_editing() {
            match code {
                KeyCode::Enter => self.profile.commit_edit(),
                KeyCode::Esc => self.profile.cancel_edit(),
                KeyCode::Backspace => self.profile.pop_char(),
                KeyCode::Char(c) => self.profile.push_char(c),
                _ => {}
            }
            return Ok(());
        }

        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.exit = true,
            KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => self.active = self.active.next(),
            KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => {
                self.active = self.active.previous()
            }
            KeyCode::Char('1') => self.active = ActiveTab::Calibration,
            KeyCode::Char('2') => self.active = ActiveTab::Observation,
            KeyCode::Char('3') => self.active = ActiveTab::Project,
            KeyCode::Char('4') => self.active = ActiveTab::Profile,
            KeyCode::Down | KeyCode::Char('j') => self.select_next(),
            KeyCode::Up | KeyCode::Char('k') => self.select_previous(),
            KeyCode::Char('r') => self.refresh()?,
            KeyCode::Enter if self.active == ActiveTab::Profile => self.profile.begin_edit(),
            KeyCode::Char('s') if self.active == ActiveTab::Profile => self.save_profile()?,
            _ => {}
        }
        Ok(())
    }

    fn select_next(&mut self) {
        match self.active {
            ActiveTab::Calibration => self.calibration.select_next(),
            ActiveTab::Observation => self.observation.select_next(),
            ActiveTab::Project => self.project.select_next(),
            ActiveTab::Profile => self.profile.select_next(),
        }
    }

    fn select_previous(&mut self) {
        match self.active {
            ActiveTab::Calibration => self.calibration.select_previous(),
            ActiveTab::Observation => self.observation.select_previous(),
            ActiveTab::Project => self.project.select_previous(),
            ActiveTab::Profile => self.profile.select_previous(),
        }
    }

    fn refresh(&mut self) -> Result<()> {
        block_on(self.calibration.load(&self.index))?;
        block_on(self.observation.load(&self.index))?;
        self.project.load(&self.index.profile.root)?;
        self.status = Some("refreshed".to_string());
        Ok(())
    }

    fn save_profile(&mut self) -> Result<()> {
        if !self.profile.is_dirty() {
            self.status = Some("no changes to save".to_string());
            return Ok(());
        }
        self.profile.apply_to(&mut self.index.config);
        match self.index.save_config() {
            Ok(()) => {
                self.profile.mark_saved();
                self.status = Some("profile saved".to_string());
            }
            Err(e) => self.status = Some(format!("save failed: {e}")),
        }
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(area);

        self.draw_tab_bar(frame, chunks[0]);
        self.draw_active_tab(frame, chunks[1]);
        self.draw_footer(frame, chunks[2]);
    }

    fn draw_tab_bar(&self, frame: &mut Frame, area: Rect) {
        let tabs = Tabs::new(ActiveTab::TITLES.to_vec())
            .select(self.active.index())
            .block(Block::bordered().title(" px tui "))
            .highlight_style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        frame.render_widget(tabs, area);
    }

    fn draw_active_tab(&mut self, frame: &mut Frame, area: Rect) {
        match self.active {
            ActiveTab::Calibration => self.calibration.draw(frame, area),
            ActiveTab::Observation => self.observation.draw(frame, area, &self.index.profile.root),
            ActiveTab::Project => self.project.draw(frame, area, &self.index.profile.root),
            ActiveTab::Profile => self.profile.draw(frame, area),
        }
    }

    fn draw_footer(&self, frame: &mut Frame, area: Rect) {
        let hint = match self.active {
            ActiveTab::Profile if self.profile.is_editing() => "Enter: commit  Esc: cancel",
            ActiveTab::Profile => {
                "Up/Down: field  Enter: edit  s: save  Tab/1-4: switch tab  r: refresh  q: quit"
            }
            _ => "Up/Down or j/k: navigate  Tab/1-4: switch tab  r: refresh  q: quit",
        };
        let text = match &self.status {
            Some(msg) => format!("{hint}   |   {msg}"),
            None => hint.to_string(),
        };
        frame.render_widget(Paragraph::new(text).block(Block::bordered()), area);
    }
}
