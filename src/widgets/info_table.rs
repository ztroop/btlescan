use ratatui::{
    layout::Constraint,
    style::{Color, Style},
    widgets::{Row, Table},
};

/// Creates a table with information about the application and the user input.
pub fn info_table(signal: bool, is_loading: &bool, frame_count: &usize) -> Table<'static> {
    let spinner = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let index = frame_count % spinner.len();
    let info_rows = vec![Row::new(vec![
        "[q → exit]".to_string(),
        "[up/down → navigate]".to_string(),
        "[enter → open/close]".to_string(),
        if *is_loading {
            format!("[loading... {}]", spinner[index])
        } else if signal {
            "[s → start scan]".to_string()
        } else {
            "[s → stop scan]".to_string()
        },
    ])
    .style(Style::default().fg(Color::DarkGray))];
    let table = Table::new(
        info_rows,
        [
            Constraint::Length(10),
            Constraint::Length(20),
            Constraint::Length(20),
            Constraint::Length(20),
        ],
    )
    .column_spacing(1);

    table
}
