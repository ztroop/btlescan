use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::structs::{Characteristic, DataFormat, InputMode};
use crate::utils::bytes_to_hex;

pub fn rw_panel<'a>(
    selected_char: Option<&Characteristic>,
    char_value: Option<&Vec<u8>>,
    data_format: &DataFormat,
    input_mode: &InputMode,
    input_buffer: &str,
    focused: bool,
    subscribed: bool,
) -> Table<'a> {
    let border_color = if focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let char_uuid = selected_char
        .map(|c| c.uuid.to_string())
        .unwrap_or_else(|| "none".to_string());

    let props = selected_char
        .map(|c| {
            c.properties
                .iter_names()
                .map(|x| x.0.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default();

    let current_value = char_value
        .map(|v| bytes_to_hex(v))
        .unwrap_or_else(|| "—".to_string());

    let sub_status = if subscribed {
        "Subscribed ●"
    } else {
        "Not subscribed"
    };

    let input_display = if *input_mode == InputMode::Editing {
        format!("▸ {}_", input_buffer)
    } else if input_buffer.is_empty() {
        "Press 'i' to enter data".to_string()
    } else {
        format!("  {}", input_buffer)
    };

    let editing_color = if *input_mode == InputMode::Editing {
        Color::Green
    } else {
        Color::White
    };

    let rows = vec![
        Row::new(vec!["Characteristic:".to_string(), char_uuid]),
        Row::new(vec!["Properties:".to_string(), props]),
        Row::new(vec!["Value:".to_string(), current_value]),
        Row::new(vec!["Notifications:".to_string(), sub_status.to_string()]),
        Row::new(vec![
            format!("Format [t]:",),
            data_format.label().to_string(),
        ]),
        Row::new(vec!["Input:".to_string(), input_display])
            .style(Style::default().fg(editing_color)),
    ];

    Table::new(rows, [Constraint::Length(18), Constraint::Fill(1)])
        .block(
            Block::default()
                .title("Read / Write")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::BOLD))
}

#[cfg(test)]
mod tests {
    use super::*;
    use btleplug::api::CharPropFlags;
    use uuid::Uuid;

    fn make_char() -> Characteristic {
        Characteristic {
            uuid: Uuid::parse_str("00002a37-0000-1000-8000-00805f9b34fb").unwrap(),
            properties: CharPropFlags::READ | CharPropFlags::WRITE | CharPropFlags::NOTIFY,
            descriptors: vec![],
            service: Uuid::parse_str("00001800-0000-1000-8000-00805f9b34fb").unwrap(),
            handle: None,
        }
    }

    #[test]
    fn test_rw_panel_no_selection() {
        let _table = rw_panel(
            None,
            None,
            &DataFormat::Hex,
            &InputMode::Normal,
            "",
            false,
            false,
        );
    }

    #[test]
    fn test_rw_panel_with_selection() {
        let ch = make_char();
        let value = vec![0x00, 0x50];
        let _table = rw_panel(
            Some(&ch),
            Some(&value),
            &DataFormat::Hex,
            &InputMode::Normal,
            "",
            true,
            false,
        );
    }

    #[test]
    fn test_rw_panel_editing() {
        let ch = make_char();
        let _table = rw_panel(
            Some(&ch),
            None,
            &DataFormat::Hex,
            &InputMode::Editing,
            "00 FF",
            true,
            true,
        );
    }
}
