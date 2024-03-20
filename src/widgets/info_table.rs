use ratatui::{
    layout::Constraint,
    style::{Color, Style},
    widgets::{Row, Table},
};

/// Creates a table with information about the application and the user input.
pub fn info_table(signal: bool) -> Table<'static> {
    let info_rows = vec![Row::new(vec![
        "[esc → quit program]",
        "[up/down → navigate]",
        "[enter → open/close]",
        if signal {
            "[s → start scanning]"
        } else {
            "[s → stop scanning]"
        },
    ])
    .style(Style::default().fg(Color::DarkGray))];
    let table = Table::new(
        info_rows,
        [
            Constraint::Length(20),
            Constraint::Length(20),
            Constraint::Length(20),
            Constraint::Length(20),
        ],
    )
    .column_spacing(1);

    table
}
