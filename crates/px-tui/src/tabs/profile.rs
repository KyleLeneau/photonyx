use px_configuration::ProfileConfig;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, List, ListItem, ListState, Padding},
};

#[derive(Clone, Copy)]
enum ProfileField {
    Name,
    Description,
    Filters,
}

const FIELDS: [ProfileField; 3] = [
    ProfileField::Name,
    ProfileField::Description,
    ProfileField::Filters,
];

impl ProfileField {
    fn label(self) -> &'static str {
        match self {
            ProfileField::Name => "Name",
            ProfileField::Description => "Description",
            ProfileField::Filters => "Filters (comma separated)",
        }
    }
}

/// Editable view over a profile's `px_profile.yaml`. Holds its own draft copies of each field so
/// edits can be typed and cancelled without touching the caller's `ProfileConfig` until
/// `apply_to` is called (on save).
#[derive(Default)]
pub struct ProfileTab {
    name: String,
    description: String,
    filters: String,
    selected: usize,
    editing: bool,
    buffer: String,
    dirty: bool,
}

impl ProfileTab {
    pub fn load(&mut self, config: &ProfileConfig) {
        self.name = config.name.clone();
        self.description = config.description.clone().unwrap_or_default();
        self.filters = config.filters.join(", ");
        self.selected = 0;
        self.editing = false;
        self.buffer.clear();
        self.dirty = false;
    }

    pub fn is_editing(&self) -> bool {
        self.editing
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn select_next(&mut self) {
        if !self.editing {
            self.selected = (self.selected + 1) % FIELDS.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.editing {
            self.selected = (self.selected + FIELDS.len() - 1) % FIELDS.len();
        }
    }

    pub fn begin_edit(&mut self) {
        self.buffer = self.field_value(self.selected).to_string();
        self.editing = true;
    }

    pub fn cancel_edit(&mut self) {
        self.editing = false;
        self.buffer.clear();
    }

    pub fn commit_edit(&mut self) {
        let value = std::mem::take(&mut self.buffer);
        match FIELDS[self.selected] {
            ProfileField::Name => self.name = value,
            ProfileField::Description => self.description = value,
            ProfileField::Filters => self.filters = value,
        }
        self.editing = false;
        self.dirty = true;
    }

    pub fn push_char(&mut self, c: char) {
        if self.editing {
            self.buffer.push(c);
        }
    }

    pub fn pop_char(&mut self) {
        if self.editing {
            self.buffer.pop();
        }
    }

    /// Write the in-memory draft back into `config`, ready for `ProfileIndex::save_config`.
    pub fn apply_to(&self, config: &mut ProfileConfig) {
        config.name = self.name.clone();
        config.description = if self.description.trim().is_empty() {
            None
        } else {
            Some(self.description.clone())
        };
        config.filters = self
            .filters
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }

    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    fn field_value(&self, idx: usize) -> &str {
        match FIELDS[idx] {
            ProfileField::Name => &self.name,
            ProfileField::Description => &self.description,
            ProfileField::Filters => &self.filters,
        }
    }

    pub fn draw(&self, frame: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = FIELDS
            .iter()
            .enumerate()
            .map(|(i, field)| {
                let raw_value = if self.editing && i == self.selected {
                    format!("{}\u{2588}", self.buffer)
                } else {
                    self.field_value(i).to_string()
                };
                let value = if raw_value.is_empty() {
                    "-".to_string()
                } else {
                    raw_value
                };
                let style = if i == self.selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                ListItem::new(format!("{}: {value}", field.label())).style(style)
            })
            .collect();

        let title = if self.dirty {
            " Profile (unsaved changes) "
        } else {
            " Profile "
        };

        let list = List::new(items).block(
            Block::bordered()
                .title(title)
                .padding(Padding::horizontal(1)),
        );

        let mut state = ListState::default();
        state.select(Some(self.selected));
        frame.render_stateful_widget(list, area, &mut state);
    }
}
