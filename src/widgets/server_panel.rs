use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::structs::{DataFormat, InputMode, ServerField};
use crate::utils::bytes_to_hex;

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
    current_value: &[u8],
    data_format: &DataFormat,
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
            format!("▸ {input_buffer}_")
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

    if is_advertising {
        rows.push(Row::new(vec![
            "Properties:".to_string(),
            "Read, Write, Notify".to_string(),
        ]));

        let value_display = if current_value.is_empty() {
            "—".to_string()
        } else {
            bytes_to_hex(current_value)
        };
        rows.push(Row::new(vec!["Value:".to_string(), value_display]));

        rows.push(Row::new(vec![
            "Format [t]:".to_string(),
            data_format.label().to_string(),
        ]));

        let input_display = if *input_mode == InputMode::Editing {
            format!("▸ {input_buffer}_")
        } else {
            "Press 'w' to enter data".to_string()
        };
        let editing_color = if *input_mode == InputMode::Editing {
            Color::Green
        } else {
            Color::White
        };
        rows.push(
            Row::new(vec!["Input:".to_string(), input_display])
                .style(Style::default().fg(editing_color)),
        );

        rows.push(Row::default());
        rows.push(
            Row::new(vec![
                String::new(),
                "[w → set value] [n → notify] [t → format] [x → stop]".to_string(),
            ])
            .style(Style::default().fg(Color::DarkGray)),
        );
    } else {
        rows.push(Row::default());
        rows.push(
            Row::new(vec![
                String::new(),
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
            &[],
            &DataFormat::Hex,
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
            &[0x00, 0x50],
            &DataFormat::Hex,
        );
    }

    #[test]
    fn test_server_panel_advertising_empty_value() {
        let _table = server_panel(
            "my-device",
            "0000180d-0000-1000-8000-00805f9b34fb",
            "00002a37-0000-1000-8000-00805f9b34fb",
            true,
            &ServerField::Name,
            &InputMode::Normal,
            "",
            true,
            &[],
            &DataFormat::Hex,
        );
    }

    #[test]
    fn test_server_panel_advertising_editing() {
        let _table = server_panel(
            "my-device",
            "0000180d-0000-1000-8000-00805f9b34fb",
            "00002a37-0000-1000-8000-00805f9b34fb",
            true,
            &ServerField::Name,
            &InputMode::Editing,
            "FF 00",
            true,
            &[0xFF],
            &DataFormat::Hex,
        );
    }

    #[test]
    fn test_server_panel_editing_config() {
        let _table = server_panel(
            "btlescan",
            "0000180d-0000-1000-8000-00805f9b34fb",
            "00002a37-0000-1000-8000-00805f9b34fb",
            false,
            &ServerField::ServiceUuid,
            &InputMode::Editing,
            "b42e2a68-ade7-11e4",
            true,
            &[],
            &DataFormat::Hex,
        );
    }
}
