use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::structs::{InputMode, ServerField};

#[allow(clippy::too_many_arguments)]
pub fn server_panel<'a>(
    server_name: &str,
    service_uuid: &str,
    char_uuid: &str,
    is_advertising: bool,
    selected_field: &ServerField,
    input_mode: &InputMode,
    input_buffer: &str,
    focused: bool,
) -> Table<'a> {
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

    let fields: [(ServerField, &str); 3] = [
        (ServerField::Name, server_name),
        (ServerField::ServiceUuid, service_uuid),
        (ServerField::CharUuid, char_uuid),
    ];

    let mut rows = vec![Row::new(vec!["Status:".to_string(), status.to_string()])
        .style(Style::default().fg(status_color))];

    for (field, value) in &fields {
        let is_selected = field == selected_field && !is_advertising;
        let is_editing = is_selected && *input_mode == InputMode::Editing;

        let display_value = if is_editing {
            format!("▸ {}_", input_buffer)
        } else {
            value.to_string()
        };

        let style = if is_editing {
            Style::default().fg(Color::Green)
        } else if is_selected {
            Style::default().add_modifier(Modifier::REVERSED)
        } else {
            Style::default()
        };

        rows.push(Row::new(vec![format!("{}:", field.label()), display_value]).style(style));
    }

    if !is_advertising {
        rows.push(Row::default());
        rows.push(
            Row::new(vec![
                "".to_string(),
                "[Enter → edit field] [a → advertise]".to_string(),
            ])
            .style(Style::default().fg(Color::DarkGray)),
        );
    }

    Table::new(rows, [Constraint::Length(18), Constraint::Fill(1)]).block(
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
        let _table = server_panel(
            "btlescan",
            "0000180d-0000-1000-8000-00805f9b34fb",
            "00002a37-0000-1000-8000-00805f9b34fb",
            false,
            &ServerField::Name,
            &InputMode::Normal,
            "",
            false,
        );
    }

    #[test]
    fn test_server_panel_advertising() {
        let _table = server_panel(
            "my-device",
            "0000180d-0000-1000-8000-00805f9b34fb",
            "00002a37-0000-1000-8000-00805f9b34fb",
            true,
            &ServerField::Name,
            &InputMode::Normal,
            "",
            true,
        );
    }

    #[test]
    fn test_server_panel_editing() {
        let _table = server_panel(
            "btlescan",
            "0000180d-0000-1000-8000-00805f9b34fb",
            "00002a37-0000-1000-8000-00805f9b34fb",
            false,
            &ServerField::ServiceUuid,
            &InputMode::Editing,
            "b42e2a68-ade7-11e4",
            true,
        );
    }
}
