use anyhow::Result;
use px_cli::{CalibrationImageType, ListMasterArgs, OutputFormat};
use px_index::{CalibrationRecord, MasterKind, ProfileIndex};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Cell, Padding, Row, Table},
};
use std::fmt::Write;
use std::io;

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn list_masters(
    args: ListMasterArgs,
    printer: Printer,
    index: ProfileIndex,
) -> Result<ExitStatus> {
    let kinds: Vec<MasterKind> = args
        .image_type
        .iter()
        .map(cli_kind_to_master_kind)
        .collect();

    let rows = if kinds.len() == 1 {
        index.list_masters(Some(kinds[0])).await?
    } else {
        index.list_masters(None).await?
    };

    let rows: Vec<&CalibrationRecord> = if kinds.len() > 1 {
        rows.iter().filter(|r| kinds.contains(&r.kind)).collect()
    } else {
        rows.iter().collect()
    };

    if rows.is_empty() {
        printer.info("no masters registered in this profile")?;
        return Ok(ExitStatus::Success);
    }

    match args.output {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&rows)?;
            writeln!(printer.stdout(), "{json}")?;
        }
        OutputFormat::Pretty => {
            render_table(&rows)?;
        }
    }

    Ok(ExitStatus::Success)
}

fn render_table(rows: &[&CalibrationRecord]) -> anyhow::Result<()> {
    let table_rows: Vec<Row> = rows
        .iter()
        .map(|row| {
            let filter = row.filter.as_deref().unwrap_or("–").to_string();
            let exposure = row
                .exposure
                .map(|e| format!("{e}s"))
                .unwrap_or_else(|| "–".to_string());
            let temp = row
                .temperature
                .map(|t| format!("{t:.1}"))
                .unwrap_or_else(|| "–".to_string());
            let gain = row
                .gain
                .map(|g| g.to_string())
                .unwrap_or_else(|| "–".to_string());
            let offset = row
                .offset
                .map(|g| g.to_string())
                .unwrap_or_else(|| "–".to_string());
            let binning = row.binning.clone().unwrap_or_else(|| "–".to_string());

            let kind_color = match row.kind {
                MasterKind::Bias => Color::Cyan,
                MasterKind::Dark => Color::Yellow,
                MasterKind::Flat => Color::Green,
            };

            Row::new([
                Cell::from(row.kind.as_str()).style(Style::default().fg(kind_color)),
                Cell::from(row.date.clone()),
                Cell::from(filter),
                Cell::from(exposure),
                Cell::from(temp),
                Cell::from(gain),
                Cell::from(offset),
                Cell::from(binning),
                Cell::from(row.master_path.clone()),
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
            table_rows.clone(),
            [
                Constraint::Length(6),
                Constraint::Length(12),
                Constraint::Length(8),
                Constraint::Length(10),
                Constraint::Length(9),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Length(8),
                Constraint::Min(20),
            ],
        )
        .block(
            Block::bordered()
                .title("Master Calibration Frames")
                .padding(Padding::horizontal(1)),
        )
        .header(
            Row::new([
                "KIND",
                "DATE",
                "FILTER",
                "EXPOSURE",
                "TEMP °C",
                "GAIN",
                "OFFSET",
                "BINNING",
                "MASTER PATH",
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

fn cli_kind_to_master_kind(k: &CalibrationImageType) -> MasterKind {
    match k {
        CalibrationImageType::Bias => MasterKind::Bias,
        CalibrationImageType::Dark => MasterKind::Dark,
        CalibrationImageType::Flat => MasterKind::Flat,
    }
}
