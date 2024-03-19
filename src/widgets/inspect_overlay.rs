use ratatui::{
    layout::Constraint,
    style::{Modifier, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::structs::Characteristic;

/// Provides an overlay with the selected device's service data.
pub fn inspect_overlay(characteristics: &[Characteristic]) -> Table<'static> {
    // Iterate through the selected device's characteristics to create rows
    let rows: Vec<Row> = characteristics
        .iter()
        .map(|characteristic| {
            let properties = format!("{:?}", characteristic.properties);
            let descriptors = characteristic
                .descriptors
                .iter()
                .map(|uuid| uuid.to_string())
                .collect::<Vec<String>>()
                .join(", ");
            Row::new(vec![
                characteristic.uuid.to_string(),
                properties,
                descriptors,
            ])
        })
        .collect();

    let table = Table::new(rows, [Constraint::Percentage(100)])
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Inspecting Device Characteristics"),
        )
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    table
}
