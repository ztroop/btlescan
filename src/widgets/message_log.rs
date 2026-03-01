use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::structs::{LogDirection, LogEntry};

pub fn message_log<'a>(
    entries: &[LogEntry],
    scroll: usize,
    height: u16,
    focused: bool,
) -> Table<'a> {
    let border_color = if focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let adjusted_height = if height > 3 { height - 3 } else { 1 };
    let visible_count = adjusted_height as usize;
    let total = entries.len();

    let start = if total > visible_count {
        scroll.min(total.saturating_sub(visible_count))
    } else {
        0
    };
    let end = (start + visible_count).min(total);

    let rows: Vec<Row> = entries[start..end]
        .iter()
        .map(|entry| {
            let color = match entry.direction {
                LogDirection::Sent => Color::Green,
                LogDirection::Received => Color::Cyan,
                LogDirection::Info => Color::DarkGray,
                LogDirection::Error => Color::Red,
            };

            Row::new(vec![format!(
                "[{}] {} {}",
                entry.timestamp,
                entry.direction.symbol(),
                entry.message
            )])
            .style(Style::default().fg(color))
        })
        .collect();

    Table::new(rows, [Constraint::Fill(1)])
        .block(
            Block::default()
                .title("Message Log")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_log_empty() {
        let entries: Vec<LogEntry> = vec![];
        let _table = message_log(&entries, 0, 10, false);
    }

    #[test]
    fn test_message_log_with_entries() {
        let entries = vec![
            LogEntry::with_timestamp("12:00:00.000", LogDirection::Info, "Connected".into()),
            LogEntry::with_timestamp("12:00:01.000", LogDirection::Received, "00 3C".into()),
            LogEntry::with_timestamp("12:00:02.000", LogDirection::Sent, "00 50".into()),
            LogEntry::with_timestamp("12:00:03.000", LogDirection::Error, "Timeout".into()),
        ];
        let _table = message_log(&entries, 0, 10, true);
    }

    #[test]
    fn test_message_log_scroll() {
        let entries: Vec<LogEntry> = (0..20)
            .map(|i| {
                LogEntry::with_timestamp("00:00:00.000", LogDirection::Info, format!("msg {i}"))
            })
            .collect();
        let _table = message_log(&entries, 15, 8, false);
    }
}
