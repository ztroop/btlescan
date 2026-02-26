use ratatui::{
    layout::Constraint,
    style::{Color, Style},
    widgets::{Row, Table},
};

use crate::structs::{AppMode, InputMode};

pub fn info_table(
    mode: &AppMode,
    input_mode: &InputMode,
    is_connected: bool,
    signal: bool,
    is_loading: &bool,
    frame_count: &usize,
) -> Table<'static> {
    let spinner = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
    let index = frame_count % spinner.len();

    let info_text = match (mode, input_mode) {
        (_, InputMode::Editing) => "[Esc → cancel] [Enter → send] [t → format]".to_string(),
        (AppMode::Client, InputMode::Normal) => {
            let mut parts = vec!["[q → exit]", "[Tab → focus]", "[m → mode]"];
            if is_connected {
                parts.extend_from_slice(&[
                    "[r → read]",
                    "[w → write]",
                    "[n → notify]",
                    "[i → input]",
                    "[d → disconnect]",
                ]);
            } else {
                parts.push("[Enter → connect]");
                if signal {
                    parts.push("[s → start scan]");
                } else {
                    parts.push("[s → stop scan]");
                }
                parts.push("[e → export]");
            }
            if *is_loading {
                parts.push("");
                let loading = format!("[loading... {}]", spinner[index]);
                return make_table(&format!("{} {}", parts.join(" "), loading));
            }
            parts.join(" ")
        }
        (AppMode::Server, InputMode::Normal) => {
            "[q → exit] [m → mode] [a → advertise] [x → stop]".to_string()
        }
    };

    make_table(&info_text)
}

fn make_table(text: &str) -> Table<'static> {
    let row = vec![Row::new(vec![text.to_string()]).style(Style::default().fg(Color::DarkGray))];
    Table::new(row, [Constraint::Fill(1)]).column_spacing(1)
}
