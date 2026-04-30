use std::{fmt::Write, io};

use anyhow::Result;

use owo_colors::OwoColorize;
use px_cli::InspectArgs;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::Constraint,
    style::{Modifier, Style},
    widgets::{Block, Cell, Padding, Row, Table},
};
use siril_sys::{
    Builder,
    commands::{Dumpheader, Stat},
    siril_ext::*,
};

use crate::{commands::ExitStatus, printer::Printer};

pub(crate) async fn inspect_file(args: InspectArgs, printer: Printer) -> Result<ExitStatus> {
    // Guard to make sure the file exists first
    if !args.file.exists() {
        writeln!(
            printer.stderr(),
            "{}",
            format_args!(
                concat!("{}{} File does not exist to inspect",),
                "error".red().bold(),
                ":".bold()
            )
        )?;
        return Ok(ExitStatus::Error);
    }

    // Startup and wait till process is ready for additional commands
    let mut siril = Builder::default()
        .output_sink(siril_sys::OutputSink::Inherit)
        .build()
        .await?;

    siril.load_path(args.file.clone()).await?;

    let stat_output = siril.execute(&Stat::builder().build()).await;
    for line in &stat_output.unwrap() {
        writeln!(printer.stdout(), "stat: {:?}", line)?;
    }

    let header_output = siril.execute(&Dumpheader::builder().build()).await;
    for line in &header_output.unwrap() {
        writeln!(printer.stdout(), "header: {:?}", line)?;
    }

    // // This dumps header and image stats to a JSON file alongside the file passed in...
    // siril
    //     .command(&format!("jsonmetadata {:?}", args.file))
    //     .await?;

    let file = px_fits::FitsFile::new(args.file.clone())?;
    let header_rows = file.header_rows();

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::with_options(
        backend,
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline((header_rows.len() + 1) as u16),
        },
    )?;

    terminal.draw(|frame| {
        let table = Table::new(
            header_rows.iter().map(|(key, val, comment)| {
                Row::new([
                    Cell::from(key.as_str()),
                    Cell::from(val.as_str()),
                    Cell::from(comment.as_str()),
                ])
            }),
            [
                Constraint::Length(20),
                Constraint::Length(30),
                Constraint::Length(100),
            ],
        )
        .block(
            Block::bordered()
                .title("FITS Headers")
                .padding(Padding::horizontal(2)),
        )
        .header(
            Row::new(["KEY", "VALUE", "COMMENT"]).style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        );
        frame.render_widget(table, frame.area());
    })?;

    Ok(ExitStatus::Success)
}
