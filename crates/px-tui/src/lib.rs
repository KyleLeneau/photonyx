mod app;
mod tabs;
mod widgets;

use anyhow::Result;
use px_index::ProfileIndex;

use crate::app::App;

/// Launch the interactive terminal UI for the given profile.
///
/// Loads the initial data for every tab up front, then hands control to a synchronous
/// `crossterm` event loop. `r` re-runs the async loads via `block_in_place`, since the loop
/// itself runs on a Tokio worker thread already inside `run`'s async context.
pub async fn run(index: ProfileIndex) -> Result<()> {
    let mut app = App::new(index);
    app.load_all().await?;

    let mut terminal = ratatui::init();
    let result = app.run(&mut terminal);
    ratatui::restore();

    result
}
