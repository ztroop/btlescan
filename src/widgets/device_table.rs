use ratatui::{
    layout::Constraint,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Row, Table},
};

use crate::structs::DeviceInfo;

pub fn device_table(selected: Option<usize>, devices: &[DeviceInfo], focused: bool) -> Table<'_> {
    let border_color = if focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let selected_style = Style::default().add_modifier(Modifier::REVERSED);
    let rows: Vec<Row> = devices
        .iter()
        .enumerate()
        .map(|(i, device)| {
            let style = if selected == Some(i) && focused {
                selected_style
            } else if selected == Some(i) {
                Style::default().fg(Color::Cyan)
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

    Table::new(
        rows,
        [
            Constraint::Length(40),
            Constraint::Length(30),
            Constraint::Length(10),
            Constraint::Length(10),
        ],
    )
    .header(
        Row::new(vec!["Identifier", "Name", "TX Power", "RSSI"])
            .style(Style::default().fg(Color::Yellow)),
    )
    .block(
        Block::default()
            .title("Detected Devices")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color)),
    )
    .row_highlight_style(selected_style)
}
