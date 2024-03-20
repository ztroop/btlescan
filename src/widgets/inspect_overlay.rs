use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::structs::Characteristic;

/// Provides an overlay with the selected device's service data.
pub fn inspect_overlay(characteristics: &[Characteristic]) -> Table<'static> {
    let mut rows: Vec<Row> = Vec::new();

    for characteristic in characteristics.iter() {
        let service_uuid = characteristic.service.to_string();
        rows.push(Row::new(vec![format!("Service: {service_uuid}")]));

        // Get flags from CharPropFlags and convert them to a string for the characteristic
        let properties = format!(
            "{:?}",
            characteristic
                .properties
                .iter_names()
                .map(|x| x.0.to_string())
                .collect::<Vec<String>>()
                .join(", ")
        );

        rows.push(Row::new(vec![format!(
            "--> {} ({})",
            characteristic.uuid.to_string(),
            properties
        )]));

        for descriptor in characteristic.descriptors.iter() {
            let descriptor_row = Row::new(vec![format!(
                "    |-- Descriptor: {}",
                descriptor.to_string()
            )]);
            rows.push(descriptor_row);
        }
    }

    let table = Table::new(rows, [Constraint::Percentage(100)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Inspecting GATT Characteristics")
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    table
}
