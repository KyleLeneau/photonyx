use ratatui::{
    layout::Constraint,
    style::{Modifier, Style},
    widgets::{Block, Padding, Row, Table},
};

/// Build a bordered, titled table with the house style shared by every list tab: bold+underlined
/// header, reversed-video row highlight, and an arrow highlight symbol.
pub fn styled_table<'a>(
    title: String,
    header: Vec<&'a str>,
    rows: Vec<Row<'a>>,
    widths: Vec<Constraint>,
) -> Table<'a> {
    Table::new(rows, widths)
        .block(
            Block::bordered()
                .title(format!(" {title} "))
                .padding(Padding::horizontal(1)),
        )
        .header(
            Row::new(header).style(
                Style::default()
                    .add_modifier(Modifier::BOLD)
                    .add_modifier(Modifier::UNDERLINED),
            ),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("> ")
}

/// Placeholder text shown in a tab that has no data loaded yet.
pub const DASH: &str = "\u{2013}";
