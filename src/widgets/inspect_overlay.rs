use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::structs::DeviceInfo;

/// Provides an overlay with the selected device's service data.
pub fn inspect_overlay(selected_device: &DeviceInfo) -> Table<'static> {
    // Iterate through the selected device's service_data to create rows
    let rows: Vec<Row> = selected_device
        .service_data
        .iter()
        .map(|(uuid, data)| {
            let data_str = data
                .iter()
                .map(|byte| format!("{:02x}", byte))
                .collect::<Vec<String>>()
                .join(" ");
            // Create a row for each UUID and its corresponding data
            Row::new(vec![uuid.to_string(), data_str])
        })
        .collect();

    let table = Table::new(
        rows,
        [Constraint::Percentage(50), Constraint::Percentage(50)],
    )
    .header(Row::new(vec!["UUID", "Data"]).style(Style::default().fg(Color::Yellow)))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title("Data Overview"),
    )
    .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    table
}
