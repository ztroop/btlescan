use ratatui::{
    layout::Constraint,
    widgets::{Block, Borders, Row, Table},
};

use crate::{structs::DeviceInfo, utils::extract_manufacturer_data};

/// Creates a table with more detailed information about a selected device.
pub fn detail_table(selected: Option<usize>, devices: &[DeviceInfo]) -> Table {
    let device_binding = DeviceInfo::default();
    let selected_device = devices
        .get(selected.unwrap_or(0))
        .unwrap_or(&device_binding);
    let services_binding = selected_device.services.len().to_string();
    let manufacturer_data = extract_manufacturer_data(&selected_device.manufacturer_data);
    let table = Table::new(
        vec![
            Row::new(vec![
                "Detected At:".to_owned(),
                selected_device.detected_at.clone(),
            ]),
            Row::new(vec!["Services:".to_owned(), services_binding]),
            Row::new(vec![
                "Company Code Identifier:".to_owned(),
                manufacturer_data.0,
            ]),
            Row::new(vec!["Manufacturer Data:".to_owned(), manufacturer_data.1]),
        ],
        [Constraint::Length(30), Constraint::Length(70)],
    )
    .block(
        Block::default()
            .title("More Detail".to_owned())
            .borders(Borders::ALL),
    );

    table
}
