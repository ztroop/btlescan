use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::Backend;
use ratatui::widgets::TableState;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::structs::DeviceInfo;
use crate::widgets::detail_table::detail_table;
use crate::widgets::device_table::device_table;
use crate::widgets::info_table::info_table;

/// Displays the detected Bluetooth devices in a table and handles the user input.
/// The user can navigate the table, pause the scanning, and quit the application.
/// The detected devices are received through the provided `mpsc::Receiver`.
pub async fn viewer<B: Backend>(
    terminal: &mut Terminal<B>,
    mut rx: mpsc::Receiver<Vec<DeviceInfo>>,
    pause_signal: Arc<AtomicBool>,
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
                .constraints(
                    [
                        Constraint::Percentage(70),
                        Constraint::Percentage(20),
                        Constraint::Percentage(10),
                    ]
                    .as_ref(),
                )
                .split(f.size());

            // Draw the device table
            let device_table = device_table(table_state.selected(), &devices);
            f.render_stateful_widget(device_table, chunks[0], &mut table_state);

            // Draw the detail table
            let detail_table = detail_table(table_state.selected(), &devices);
            f.render_widget(detail_table, chunks[1]);

            // Draw the info table
            let info_table = info_table(pause_signal.load(Ordering::SeqCst));
            f.render_widget(info_table, chunks[2]);
        })?;

        // Event handling
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('s') => {
                        let current_state = pause_signal.load(Ordering::SeqCst);
                        pause_signal.store(!current_state, Ordering::SeqCst);
                    }
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
