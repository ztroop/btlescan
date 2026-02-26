use ratatui::{
    layout::Constraint,
    style::{Color, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::{structs::DeviceInfo, utils::extract_manufacturer_data};

pub fn detail_table(selected_device: &DeviceInfo, is_connected: bool) -> Table<'_> {
    let services_binding = selected_device.services.len().to_string();
    let manufacturer_data = extract_manufacturer_data(&selected_device.manufacturer_data);

    let connection_status = if is_connected {
        "Connected ●".to_string()
    } else {
        "Disconnected".to_string()
    };

    let status_color = if is_connected {
        Color::Green
    } else {
        Color::DarkGray
    };

    let rows = vec![
        Row::new(vec!["Status:".to_owned(), connection_status])
            .style(Style::default().fg(status_color)),
        Row::new(vec![
            "Detected At:".to_owned(),
            selected_device.detected_at.clone(),
        ]),
        Row::new(vec!["Services:".to_owned(), services_binding]),
        Row::new(vec![
            "Company Code ID:".to_owned(),
            manufacturer_data.company_code,
        ]),
        Row::new(vec![
            "Manufacturer Data:".to_owned(),
            manufacturer_data.data,
        ]),
    ];

    Table::new(rows, [Constraint::Length(20), Constraint::Length(80)])
        .block(Block::default().title("Details").borders(Borders::ALL))
}
