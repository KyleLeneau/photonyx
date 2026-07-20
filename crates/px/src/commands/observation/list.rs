use anyhow::Result;
use chrono::{Duration, Utc};
use px_cli::{ListObservationArgs, OutputFormat};
use px_index::{ObservationWithMasters, ProfileIndex};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Cell, Padding, Row, Table},
};
use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn list_observations(
    args: ListObservationArgs,
    printer: Printer,
    index: ProfileIndex,
) -> Result<ExitStatus> {
    let all = index.list_observations_with_masters().await?;

    let cutoff = args.days.map(|d| {
        (Utc::now().date_naive() - Duration::days(d as i64))
            .format("%Y-%m-%d")
            .to_string()
    });

    let rows: Vec<&ObservationWithMasters> = all
        .iter()
        .filter(|r| apply_filters(r, &args, cutoff.as_deref()))
        .collect();

    if rows.is_empty() {
        printer.info("no observations found matching the given filters")?;
        return Ok(ExitStatus::Success);
    }

    match args.output {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rows)?;
            writeln!(printer.stdout(), "{json}")?;
        }
        OutputFormat::Pretty => {
            render_table(&rows, &index.profile.root)?;
        }
    }

    Ok(ExitStatus::Success)
}

fn apply_filters(
    r: &ObservationWithMasters,
    args: &ListObservationArgs,
    cutoff: Option<&str>,
) -> bool {
    if let Some(ref t) = args.target
        && !r.target_name.to_lowercase().contains(&t.to_lowercase())
    {
        return false;
    }
    if let Some(c) = cutoff
        && r.date.as_str() < c
    {
        return false;
    }
    if let Some(ref from) = args.from
        && r.date.as_str() < from.as_str()
    {
        return false;
    }
    if let Some(ref to) = args.to
        && r.date.as_str() > to.as_str()
    {
        return false;
    }
    if let Some(ref f) = args.filter
        && !r.filter.to_lowercase().contains(&f.to_lowercase())
    {
        return false;
    }
    if args.calibrated && r.calibrated_path.is_none() {
        return false;
    }
    if args.uncalibrated && r.calibrated_path.is_some() {
        return false;
    }
    if let Some(min) = args.min_frames {
        match r.frame_count {
            Some(fc) if fc >= min as i64 => {}
            _ => return false,
        }
    }
    true
}

fn file_stem(path: &str) -> &str {
    Path::new(path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(path)
}

fn relative_path<'a>(raw_path: &'a str, profile_root: &Path) -> std::borrow::Cow<'a, str> {
    Path::new(raw_path)
        .strip_prefix(profile_root)
        .ok()
        .and_then(|p| p.to_str())
        .map(|s| std::borrow::Cow::Owned(s.to_string()))
        .unwrap_or(std::borrow::Cow::Borrowed(raw_path))
}

fn render_table(rows: &[&ObservationWithMasters], profile_root: &Path) -> anyhow::Result<()> {
    let dash = "\u{2013}";

    let table_rows: Vec<Row> = rows
        .iter()
        .map(|r| {
            let exposure = format!("{:.0}s", r.exposure);
            let frames = r
                .frame_count
                .map(|f| f.to_string())
                .unwrap_or_else(|| dash.to_string());
            let (cal_symbol, cal_color) = if r.calibrated_path.is_some() {
                ("\u{2713}", Color::Green)
            } else {
                ("\u{2717}", Color::Red)
            };
            let bias = r.bias_path.as_deref().map(file_stem).unwrap_or(dash);
            let dark = r.dark_path.as_deref().map(file_stem).unwrap_or(dash);
            let flat = r.flat_path.as_deref().map(file_stem).unwrap_or(dash);
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
                Cell::from(path.into_owned()),
            ])
        })
        .collect();

    let count = rows.len();
    let height = (count + 4).min(60) as u16;

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::with_options(
        backend,
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(height),
        },
    )?;

    terminal.draw(|frame| {
        let table = Table::new(
            table_rows.clone(),
            [
                Constraint::Length(18),
                Constraint::Length(12),
                Constraint::Length(12),
                Constraint::Length(8),
                Constraint::Length(6),
                Constraint::Length(4),
                Constraint::Length(35),
                Constraint::Length(35),
                Constraint::Length(35),
                Constraint::Min(20),
            ],
        )
        .block(
            Block::bordered()
                .title(format!(" Observations ({count}) "))
                .padding(Padding::horizontal(1)),
        )
        .header(
            Row::new([
                "TARGET", "DATE", "FILTER", "EXPOSURE", "FRAMES", "CAL", "BIAS", "DARK", "FLAT",
                "PATH",
            ])
            .style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        );
        frame.render_widget(table, frame.area());
    })?;

    Ok(())
}
