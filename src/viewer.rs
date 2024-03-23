use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::Backend;
use ratatui::layout::Alignment;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear, Paragraph, TableState};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::scan::get_characteristics;
use crate::structs::{App, DeviceInfo};
use crate::utils::centered_rect;
use crate::widgets::detail_table::detail_table;
use crate::widgets::device_table::device_table;
use crate::widgets::info_table::info_table;
use crate::widgets::inspect_overlay::inspect_overlay;

/// Displays the detected Bluetooth devices in a table and handles the user input.
/// The user can navigate the table, pause the scanning, and quit the application.
/// The detected devices are received through the provided `mpsc::Receiver`.
pub async fn viewer<B: Backend>(
    terminal: &mut Terminal<B>,
    mut rx: mpsc::Receiver<Vec<DeviceInfo>>,
    pause_signal: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    let mut app = App {
        table_state: TableState::default(),
        devices: Vec::<DeviceInfo>::new(),
        inspect_view: false,
        inspect_overlay_scroll: 0,
        selected_characteristics: Vec::new(),
        frame_count: 0,
        is_loading: false,
        error_view: false,
        error_message: String::new(),
    };
    app.table_state.select(Some(0));

    loop {
        // Draw UI
        terminal.draw(|f| {
            app.frame_count = f.count();
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

            let device_binding = &DeviceInfo::default();
            let selected_device = app
                .devices
                .get(app.table_state.selected().unwrap_or(0))
                .unwrap_or(device_binding);

            // Draw the device table
            let device_table = device_table(app.table_state.selected(), &app.devices);
            f.render_stateful_widget(device_table, chunks[0], &mut app.table_state);

            // Draw the detail table
            let detail_table = detail_table(selected_device);
            f.render_widget(detail_table, chunks[1]);

            // Draw the info table
            app.frame_count += 1;
            let info_table = info_table(
                pause_signal.load(Ordering::SeqCst),
                &app.is_loading,
                &app.frame_count,
            );
            f.render_widget(info_table, chunks[2]);

            // Draw the inspect overlay
            if app.inspect_view {
                let area = centered_rect(60, 60, f.size());
                let inspect_overlay = inspect_overlay(
                    &app.selected_characteristics,
                    app.inspect_overlay_scroll,
                    area.height,
                );
                f.render_widget(Clear, area);
                f.render_widget(inspect_overlay, area);
            }

            // Draw the error overlay
            if app.error_view {
                let error_message_clone = app.error_message.clone();
                let area = centered_rect(60, 10, f.size());
                let error_block = Paragraph::new(Span::from(error_message_clone))
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL).title("Notification"));
                f.render_widget(Clear, area);
                f.render_widget(error_block, area);
            }
        })?;

        // Event handling
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        break;
                    }
                    KeyCode::Char('s') => {
                        let current_state = pause_signal.load(Ordering::SeqCst);
                        pause_signal.store(!current_state, Ordering::SeqCst);
                    }
                    KeyCode::Enter => {
                        if app.error_view {
                            app.error_view = false;
                        } else if app.inspect_view {
                            app.inspect_view = false;
                        } else {
                            app.is_loading = true;
                            let device_binding = &DeviceInfo::default();
                            let selected_device = app
                                .devices
                                .get(app.table_state.selected().unwrap_or(0))
                                .unwrap_or(device_binding);
                            pause_signal.store(true, Ordering::SeqCst);
                            match get_characteristics(&selected_device.device.clone().unwrap())
                                .await
                            {
                                Ok(characteristics) => {
                                    app.selected_characteristics = characteristics;
                                    app.inspect_view = !app.inspect_view;
                                }
                                Err(e) => {
                                    app.error_message =
                                        format!("Error getting characteristics: {}", e);
                                    app.error_view = true;
                                }
                            }
                            app.is_loading = false;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        if app.inspect_view {
                            app.inspect_overlay_scroll += 1;
                        } else {
                            let next = match app.table_state.selected() {
                                Some(selected) => {
                                    if selected >= app.devices.len() - 1 {
                                        0
                                    } else {
                                        selected + 1
                                    }
                                }
                                None => 0,
                            };
                            app.table_state.select(Some(next));
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        if app.inspect_view {
                            app.inspect_overlay_scroll =
                                app.inspect_overlay_scroll.saturating_sub(1);
                        } else {
                            let previous = match app.table_state.selected() {
                                Some(selected) => {
                                    if selected == 0 {
                                        app.devices.len() - 1
                                    } else {
                                        selected - 1
                                    }
                                }
                                None => 0,
                            };
                            app.table_state.select(Some(previous));
                        }
                    }
                    _ => {}
                }
            }
        }

        // Check for new devices
        if let Ok(new_devices) = rx.try_recv() {
            app.devices = new_devices;
            if app.table_state.selected().is_none() {
                app.table_state.select(Some(0));
            }
        }
    }
    Ok(())
}
