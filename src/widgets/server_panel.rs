use ratatui::{
    layout::Constraint,
    style::{Color, Style},
    widgets::{Block, Borders, Row, Table},
};

pub fn server_panel<'a>(server_name: &str, is_advertising: bool, focused: bool) -> Table<'a> {
    let border_color = if focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let status = if is_advertising {
        "Advertising"
    } else {
        "Stopped"
    };

    let status_color = if is_advertising {
        Color::Green
    } else {
        Color::Red
    };

    let rows = vec![
        Row::new(vec!["Device Name:".to_string(), server_name.to_string()]),
        Row::new(vec!["Status:".to_string(), status.to_string()])
            .style(Style::default().fg(status_color)),
        Row::default(),
        Row::new(vec![
            "Note:".to_string(),
            "GATT server requires platform-specific support.".to_string(),
        ])
        .style(Style::default().fg(Color::DarkGray)),
        Row::new(vec![
            "".to_string(),
            "Use 'a' to advertise, 'x' to stop.".to_string(),
        ])
        .style(Style::default().fg(Color::DarkGray)),
    ];

    Table::new(rows, [Constraint::Length(15), Constraint::Fill(1)]).block(
        Block::default()
            .title("GATT Server")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_panel_stopped() {
        let _table = server_panel("btlescan", false, false);
    }

    #[test]
    fn test_server_panel_advertising() {
        let _table = server_panel("my-device", true, true);
    }
}
