use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::Backend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::{BarChart, TableState};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Row, Table},
    Terminal,
};
use std::error::Error;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::structs::DeviceInfo;

pub async fn viewer<B: Backend>(
    terminal: &mut Terminal<B>,
    mut rx: mpsc::Receiver<Vec<DeviceInfo>>,
) -> Result<(), Box<dyn Error>> {
    let mut table_state = TableState::default();
    table_state.select(Some(0));
    let mut devices = Vec::<DeviceInfo>::new();
    let mut device_count_history: Vec<(String, u64)> = Vec::new();
    let max_history_points: usize = 10;

    loop {
        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)].as_ref())
                .split(f.size());

            let selected_style = Style::default().add_modifier(Modifier::REVERSED);
            let rows: Vec<Row> = devices
                .iter()
                .enumerate()
                .map(|(i, device)| {
                    let style = if table_state.selected() == Some(i) {
                        selected_style
                    } else {
                        Style::default()
                    };
                    Row::new(vec![
                        device.address.clone(),
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
                    Constraint::Length(30),
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

            f.render_stateful_widget(table, chunks[0], &mut table_state);

            let barchart_data: Vec<(&str, u64)> = device_count_history
                .iter()
                .map(|(time, count)| (time.as_str(), *count))
                .collect();
            let barchart = BarChart::default()
                .block(
                    Block::default()
                        .title("Devices Over Time")
                        .borders(Borders::ALL),
                )
                .data(&barchart_data) // Use the adjusted data here
                .bar_width(3)
                .bar_gap(2)
                .bar_style(Style::default().fg(Color::Cyan))
                .value_style(
                    Style::default()
                        .fg(Color::White)
                        .bg(Color::Black)
                        .add_modifier(Modifier::BOLD),
                );

            f.render_widget(barchart, chunks[1]);
        })?;

        // Event handling
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Down => {
                        let next = match table_state.selected() {
                            Some(selected) => {
                                if selected >= devices.len() - 1 {
                                    0
                                } else {
                                    selected + 1
                                }
                            }
                            None => 0,
                        };
                        table_state.select(Some(next));
                    }
                    KeyCode::Up => {
                        let previous = match table_state.selected() {
                            Some(selected) => {
                                if selected == 0 {
                                    devices.len() - 1
                                } else {
                                    selected - 1
                                }
                            }
                            None => 0,
                        };
                        table_state.select(Some(previous));
                    }
                    _ => {}
                }
            }
        }

        // Check for new devices
        if let Ok(new_devices) = rx.try_recv() {
            devices = new_devices;
            // Update the device count history
            device_count_history.push(("Time".to_string(), devices.len() as u64));
            // Ensure we keep only the latest `max_history_points` entries
            if device_count_history.len() > max_history_points {
                device_count_history.remove(0);
            }
        }
    }
    Ok(())
}
