use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::structs::DeviceInfo;

/// Creates a table with the detected BTLE devices.
pub fn device_table(selected: Option<usize>, devices: &[DeviceInfo]) -> Table {
    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let rows: Vec<Row> = devices
        .iter()
        .enumerate()
        .map(|(i, device)| {
            let style = if selected == Some(i) {
                selected_style
            } else {
                Style::default()
            };
            Row::new(vec![
                device.get_id(),
                device.name.clone(),
                device.tx_power.clone(),
                device.rssi.clone(),
            ])
            .style(style)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(40),
            Constraint::Length(30),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Address", "Name", "TX Power", "RSSI"])
            .style(Style::default().fg(Color::Yellow)),
    )
    .block(
        Block::default()
            .title("Detected Bluetooth Devices")
            .borders(Borders::ALL),
    )
    .highlight_style(selected_style);

    table
}
