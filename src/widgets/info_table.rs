use ratatui::{
    layout::Constraint,
    style::{Color, Style},
    widgets::{Row, Table},
};

/// Creates a table with information about the application and the user input.
pub fn info_table(signal: bool, is_loading: &bool, frame_count: &usize) -> Table<'static> {
    let spinner = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let index = frame_count % spinner.len();
    let info_text = format!(
        "[q → exit] [c → csv] [up/down → navigate] [enter → open/close] {}",
        if *is_loading {
            format!("[loading... {}]", spinner[index])
        } else if signal {
            "[s → start scan]".to_string()
        } else {
            "[s → stop scan]".to_string()
        }
    );

    let info_row = vec![Row::new(vec![info_text]).style(Style::default().fg(Color::DarkGray))];
    let table = Table::new(info_row, [Constraint::Fill(1)]).column_spacing(1);

    table
}
