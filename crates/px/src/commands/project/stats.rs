use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;

use anyhow::Result;
use chrono::NaiveDate;
use px_cli::{OutputFormat, StatsProjectArgs};
use px_configuration::{
    Framing, FramingLock, GridMosiacFraming, ObservationEntry, ProjectLock, SingleFraming,
    SpiralMosiacFraming, hash_linear_stack, hash_spiral_framing,
};
use px_conventions::{observation::ObservationPath, project::ProjectPath};
use px_fits::all_fits_files;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    border,
    layout::Constraint,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Cell, Padding, Paragraph, Row, Table},
};
use serde::Serialize;

use crate::{ExitStatus, printer::Printer};

/// One parsed observation folder, reduced to what stats need.
struct ObservationAgg {
    filter: Option<String>,
    sub_count: usize,
    total_exposure_secs: f64,
    date: Option<NaiveDate>,
}

#[derive(Debug, Serialize)]
struct LayerStats {
    name: String,
    filter: Option<String>,
    sub_count: usize,
    total_exposure_secs: f64,
    avg_sub_exposure_secs: f64,
    nights: usize,
    first_light: Option<String>,
    last_light: Option<String>,
    up_to_date: bool,
    stacked_at: Option<String>,
}

#[derive(Debug, Serialize)]
struct FilterStats {
    filter: String,
    sub_count: usize,
    total_exposure_secs: f64,
    percent_of_total: f64,
}

#[derive(Debug, Serialize)]
struct ProjectStats {
    name: String,
    description: Option<String>,
    framing: String,
    layers: Vec<LayerStats>,
    filters: Vec<FilterStats>,
    total_exposure_secs: f64,
    total_subs: usize,
    nights: usize,
    first_light: Option<String>,
    last_light: Option<String>,
    layers_needing_restack: usize,
}

pub(crate) async fn show_project_stats(
    args: StatsProjectArgs,
    printer: Printer,
) -> Result<ExitStatus> {
    let project = match ProjectPath::find(args.project) {
        Ok(path) => path,
        Err(e) => {
            printer.error(format!("{e}"))?;
            return Ok(ExitStatus::Failure);
        }
    };

    let config = project.load_config()?;
    let lock = ProjectLock::load(&project.root)?;

    let (layers, all_aggs) = match &config.framing {
        Framing::Single(single) => single_layer_stats(single, lock.as_ref())?,
        Framing::SpiralMosiac(spiral) => spiral_layer_stats(spiral, lock.as_ref())?,
        Framing::GridMosiac(grid) => grid_layer_stats(grid, lock.as_ref())?,
    };

    let stats = build_project_stats(
        config.name.clone(),
        config.description.clone(),
        &config.framing,
        layers,
        &all_aggs,
    );

    match args.output {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&stats)?;
            writeln!(printer.stdout(), "{json}")?;
        }
        OutputFormat::Pretty => {
            render_summary(&stats)?;
            render_layers_table(&stats)?;
        }
    }

    Ok(ExitStatus::Success)
}

/// Parse each observation folder and count its sub-frames on disk.
fn aggregate_observations(entries: &[ObservationEntry]) -> Result<Vec<ObservationAgg>> {
    entries
        .iter()
        .map(|entry| {
            let op = ObservationPath::single(&entry.path)?;
            let sub_count = all_fits_files(op.raw_path())?.len();
            let exposure = op.exposure().unwrap_or(0.0);
            Ok(ObservationAgg {
                filter: op.filter().map(str::to_string),
                sub_count,
                total_exposure_secs: exposure * sub_count as f64,
                date: op.date(),
            })
        })
        .collect()
}

fn summarize_layer(
    name: String,
    filter_hint: Option<String>,
    aggs: &[ObservationAgg],
    up_to_date: bool,
    stacked_at: Option<String>,
) -> LayerStats {
    let sub_count: usize = aggs.iter().map(|a| a.sub_count).sum();
    let total_exposure_secs: f64 = aggs.iter().map(|a| a.total_exposure_secs).sum();

    let mut dates: Vec<NaiveDate> = aggs.iter().filter_map(|a| a.date).collect();
    dates.sort();
    dates.dedup();

    let avg_sub_exposure_secs = if sub_count > 0 {
        total_exposure_secs / sub_count as f64
    } else {
        0.0
    };

    let filter = filter_hint.or_else(|| aggs.first().and_then(|a| a.filter.clone()));

    LayerStats {
        name,
        filter,
        sub_count,
        total_exposure_secs,
        avg_sub_exposure_secs,
        nights: dates.len(),
        first_light: dates.first().map(NaiveDate::to_string),
        last_light: dates.last().map(NaiveDate::to_string),
        up_to_date,
        stacked_at,
    }
}

fn single_layer_stats(
    single: &SingleFraming,
    lock: Option<&ProjectLock>,
) -> Result<(Vec<LayerStats>, Vec<ObservationAgg>)> {
    let old_single = lock.and_then(|l| match &l.framing {
        FramingLock::Single(s) => Some(s),
        _ => None,
    });

    let mut layers = Vec::new();
    let mut all_aggs = Vec::new();

    for stack in &single.master_lights {
        let aggs = aggregate_observations(&stack.observations)?;

        let hash = hash_linear_stack(stack);
        let layer_lock = old_single.and_then(|s| s.find_layer(&stack.name));
        let up_to_date = layer_lock.is_some_and(|l| !l.is_dirty(&hash));
        let stacked_at = layer_lock.and_then(|l| l.stacked_at.clone());

        layers.push(summarize_layer(
            stack.name.clone(),
            stack.filter.clone(),
            &aggs,
            up_to_date,
            stacked_at,
        ));
        all_aggs.extend(aggs);
    }

    Ok((layers, all_aggs))
}

fn spiral_layer_stats(
    spiral: &SpiralMosiacFraming,
    lock: Option<&ProjectLock>,
) -> Result<(Vec<LayerStats>, Vec<ObservationAgg>)> {
    let old_spiral = lock.and_then(|l| match &l.framing {
        FramingLock::SpiralMosiac(s) => Some(s),
        _ => None,
    });

    let aggs = aggregate_observations(&spiral.observations)?;

    let hash = hash_spiral_framing(spiral);
    let up_to_date = old_spiral.is_some_and(|s| !s.is_dirty(&hash));
    let stacked_at = old_spiral.and_then(|s| s.stacked_at.clone());

    let layer = summarize_layer(
        spiral.name.clone(),
        spiral.filter.clone(),
        &aggs,
        up_to_date,
        stacked_at,
    );

    Ok((vec![layer], aggs))
}

fn grid_layer_stats(
    grid: &GridMosiacFraming,
    lock: Option<&ProjectLock>,
) -> Result<(Vec<LayerStats>, Vec<ObservationAgg>)> {
    let old_grid = lock.and_then(|l| match &l.framing {
        FramingLock::GridMosiac(g) => Some(g),
        _ => None,
    });

    let mut layers = Vec::new();
    let mut all_aggs = Vec::new();

    for grid_layer in &grid.master_lights {
        let mut aggs = Vec::new();
        for panel in &grid_layer.panels {
            aggs.extend(aggregate_observations(&panel.observations)?);
        }

        let old_layer = old_grid.and_then(|g| g.find_layer(&grid_layer.name));
        let up_to_date = old_layer.is_some_and(|l| {
            !l.is_grid_dirty()
                && grid_layer.panels.iter().all(|panel| {
                    let hash = hash_linear_stack(panel);
                    l.find_panel(&panel.name)
                        .is_some_and(|p| !p.is_dirty(&hash))
                })
        });
        let stacked_at = old_layer.and_then(|l| l.stacked_at.clone());

        layers.push(summarize_layer(
            grid_layer.name.clone(),
            grid_layer.filter.clone(),
            &aggs,
            up_to_date,
            stacked_at,
        ));
        all_aggs.extend(aggs);
    }

    Ok((layers, all_aggs))
}

fn build_project_stats(
    name: String,
    description: Option<String>,
    framing: &Framing,
    layers: Vec<LayerStats>,
    all_aggs: &[ObservationAgg],
) -> ProjectStats {
    let total_exposure_secs: f64 = layers.iter().map(|l| l.total_exposure_secs).sum();
    let total_subs: usize = layers.iter().map(|l| l.sub_count).sum();
    let layers_needing_restack = layers.iter().filter(|l| !l.up_to_date).count();

    let mut dates: Vec<NaiveDate> = all_aggs.iter().filter_map(|a| a.date).collect();
    dates.sort();
    dates.dedup();

    let mut by_filter: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    for agg in all_aggs {
        let key = agg.filter.clone().unwrap_or_else(|| "unknown".to_string());
        let entry = by_filter.entry(key).or_insert((0, 0.0));
        entry.0 += agg.sub_count;
        entry.1 += agg.total_exposure_secs;
    }

    let filters = by_filter
        .into_iter()
        .map(|(filter, (sub_count, exposure))| FilterStats {
            filter,
            sub_count,
            total_exposure_secs: exposure,
            percent_of_total: if total_exposure_secs > 0.0 {
                exposure / total_exposure_secs * 100.0
            } else {
                0.0
            },
        })
        .collect();

    ProjectStats {
        name,
        description,
        framing: framing.kind_str().to_string(),
        layers,
        filters,
        total_exposure_secs,
        total_subs,
        nights: dates.len(),
        first_light: dates.first().map(NaiveDate::to_string),
        last_light: dates.last().map(NaiveDate::to_string),
        layers_needing_restack,
    }
}

fn render_layers_table(stats: &ProjectStats) -> Result<()> {
    let rows: Vec<Row> = stats
        .layers
        .iter()
        .map(|layer| {
            let status = if layer.up_to_date {
                Cell::from("up to date").style(Style::default().fg(Color::Green))
            } else {
                Cell::from("needs restack").style(Style::default().fg(Color::Yellow))
            };
            let span = match (&layer.first_light, &layer.last_light) {
                (Some(first), Some(last)) if first == last => first.clone(),
                (Some(first), Some(last)) => format!("{first} .. {last}"),
                _ => "-".to_string(),
            };

            Row::new([
                Cell::from(layer.name.clone()),
                Cell::from(layer.filter.clone().unwrap_or_else(|| "-".to_string())),
                Cell::from(layer.sub_count.to_string()),
                Cell::from(format_duration(layer.total_exposure_secs)),
                Cell::from(format!("{:.0}s", layer.avg_sub_exposure_secs)),
                Cell::from(layer.nights.to_string()),
                Cell::from(span),
                status,
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
            rows.clone(),
            [
                Constraint::Length(16),
                Constraint::Length(10),
                Constraint::Length(6),
                Constraint::Length(10),
                Constraint::Length(9),
                Constraint::Length(7),
                Constraint::Length(26),
                Constraint::Length(14),
            ],
        )
        .block(
            Block::bordered()
                .title(" Project Layers ")
                .padding(Padding::horizontal(1)),
        )
        .header(
            Row::new([
                "LAYER", "FILTER", "SUBS", "EXPOSURE", "AVG SUB", "NIGHTS", "SPAN", "STATUS",
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

const FILTER_BAR_WIDTH: usize = 24;

fn render_summary(stats: &ProjectStats) -> Result<()> {
    let lines = summary_lines(stats);

    let height = lines.len() as u16 + 4;
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::with_options(
        backend,
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(height),
        },
    )?;

    terminal.draw(|frame| {
        let paragraph = Paragraph::new(lines.clone()).block(
            Block::bordered()
                .borders(border!(TOP, LEFT, RIGHT))
                .title(" Summary ")
                .padding(Padding::uniform(1)),
        );
        frame.render_widget(paragraph, frame.area());
    })?;

    Ok(())
}

fn summary_lines(stats: &ProjectStats) -> Vec<Line<'static>> {
    let label_style = Style::default().fg(Color::DarkGray);
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(vec![
        Span::styled("target", label_style),
        Span::raw(": "),
        Span::styled(
            stats.name.clone(),
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::Cyan),
        ),
    ]));

    if let Some(desc) = stats.description.clone() {
        lines.push(Line::from(vec![
            Span::styled("description", label_style),
            Span::raw(": "),
            Span::styled(desc, Style::default().fg(Color::Cyan)),
        ]));
    }

    lines.push(Line::from(vec![
        Span::styled("framing", label_style),
        Span::raw(": "),
        Span::styled(
            stats.framing.clone(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw("    "),
        Span::styled("total", label_style),
        Span::raw(": "),
        Span::styled(
            format_duration(stats.total_exposure_secs),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            " across {} subs over {} night(s)",
            stats.total_subs, stats.nights
        )),
    ]));

    if let (Some(first), Some(last)) = (&stats.first_light, &stats.last_light) {
        lines.push(Line::from(vec![
            Span::styled("first light", label_style),
            Span::raw(": "),
            Span::styled(first.clone(), Style::default().fg(Color::Yellow)),
            Span::raw("    "),
            Span::styled("last light", label_style),
            Span::raw(": "),
            Span::styled(last.clone(), Style::default().fg(Color::Yellow)),
        ]));
    }

    if stats.layers_needing_restack > 0 {
        lines.push(Line::from(Span::styled(
            format!(
                "{} layer(s) need restacking — run `px project stack`",
                stats.layers_needing_restack
            ),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )));
    }

    if !stats.filters.is_empty() {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            "filter balance",
            Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        )));

        let name_width = stats
            .filters
            .iter()
            .map(|f| f.filter.len())
            .max()
            .unwrap_or(0);
        for f in &stats.filters {
            let filled = ((f.percent_of_total / 100.0) * FILTER_BAR_WIDTH as f64).round() as usize;
            let filled = filled.min(FILTER_BAR_WIDTH);
            let bar = format!(
                "{}{}",
                "█".repeat(filled),
                "░".repeat(FILTER_BAR_WIDTH - filled)
            );

            lines.push(Line::from(vec![
                Span::styled(
                    format!("{:<name_width$}", f.filter),
                    Style::default().fg(Color::Magenta),
                ),
                Span::raw("  "),
                Span::styled(bar, Style::default().fg(Color::Cyan)),
                Span::raw(format!("  {:>5.1}%  ", f.percent_of_total)),
                Span::styled(
                    format_duration(f.total_exposure_secs),
                    Style::default().fg(Color::DarkGray),
                ),
            ]));
        }
    }

    lines
}

fn format_duration(total_secs: f64) -> String {
    let total_secs = total_secs.round() as i64;
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    if hours > 0 {
        format!("{hours}h {minutes:02}m")
    } else {
        format!("{minutes}m")
    }
}
