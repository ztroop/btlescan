use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::Backend;
use ratatui::style::{Color, Modifier, Style};
use ratatui::widgets::TableState;
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

    loop {
        // Draw UI
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Percentage(100)].as_ref())
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
                    let device_address = if device.address == "00:00:00:00:00:00" {
                        device.id.clone()
                    } else {
                        device.address.clone()
                    };
                    Row::new(vec![
                        device_address,
                        device.name.clone(),
                        device.tx_power.clone(),
                        device.rssi.clone(),
                        device.detected_at.clone(),
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
                    Constraint::Length(20),
                ],
            )
            .header(
                Row::new(vec!["Address", "Name", "TX Power", "RSSI", "Detected At"])
                    .style(Style::default().fg(Color::Yellow)),
            )
            .block(
                Block::default()
                    .title("Detected Bluetooth Devices")
                    .borders(Borders::ALL),
            )
            .highlight_style(selected_style);

            f.render_stateful_widget(table, chunks[0], &mut table_state);
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
            if table_state.selected().is_none() {
                table_state.select(Some(0));
            }
        }
    }
    Ok(())
}
