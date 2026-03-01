use crossterm::event::{self, Event, KeyCode};
use ratatui::backend::Backend;
use ratatui::layout::Alignment;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use std::error::Error;
use std::sync::atomic::Ordering;
use std::time::Duration;

use crate::app::{App, DeviceData, MAX_DEVICES};
#[cfg(feature = "server")]
use crate::structs::ServerField;
use crate::structs::{AppMode, FocusPanel, InputMode, LogDirection};
use crate::utils::{bytes_to_hex, centered_rect};
use crate::widgets::characteristic_panel::characteristic_panel;
use crate::widgets::detail_table::detail_table;
use crate::widgets::device_table::device_table;
use crate::widgets::info_table::info_table;
use crate::widgets::message_log::message_log;
use crate::widgets::rw_panel::rw_panel;
#[cfg(feature = "server")]
use crate::widgets::server_panel::server_panel;

pub async fn viewer<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), Box<dyn Error>>
where
    <B as Backend>::Error: 'static,
{
    if !app.devices.is_empty() {
        app.table_state.select(Some(0));
    }

    loop {
        terminal.draw(|f| {
            app.frame_count = f.count();

            match app.mode {
                AppMode::Client => draw_client_mode(f, app),
                #[cfg(feature = "server")]
                AppMode::Server => draw_server_mode(f, app),
            }

            if app.error_view {
                let error_message_clone = app.error_message.clone();
                let area = centered_rect(60, 10, f.area());
                let error_block = Paragraph::new(Span::from(error_message_clone))
                    .alignment(Alignment::Center)
                    .block(Block::default().borders(Borders::ALL).title("Notification"));
                f.render_widget(Clear, area);
                f.render_widget(error_block, area);
            }
        })?;

        if event::poll(Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if app.error_view {
                        if matches!(key.code, KeyCode::Enter | KeyCode::Esc) {
                            app.error_view = false;
                        }
                        continue;
                    }

                    if app.input_mode == InputMode::Editing {
                        handle_editing_input(app, key.code);
                        continue;
                    }

                    match app.mode {
                        AppMode::Client => handle_client_input(app, key.code).await,
                        #[cfg(feature = "server")]
                        AppMode::Server => handle_server_input(app, key.code).await,
                    }

                    if app.should_quit {
                        let disconnect_handle = app
                            .connected_device
                            .is_some()
                            .then(|| app.disconnect())
                            .flatten();
                        app.stop_scan().await;
                        #[cfg(feature = "server")]
                        if app.is_advertising {
                            app.stop_server().await;
                        }
                        if let Some(h) = disconnect_handle {
                            let _ = h.await;
                        }
                        return Ok(());
                    }
                }
                Event::Paste(data) => {
                    if app.input_mode == InputMode::Editing {
                        for c in data.chars() {
                            app.insert_char(c);
                        }
                    }
                }
                _ => {}
            }
        }

        let was_loading = app.is_loading;
        process_channel_messages(app);
        if was_loading && !app.is_loading && !app.is_connected {
            app.scan();
        }
    }
}

fn draw_client_mode(f: &mut ratatui::Frame, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Percentage(45),
            Constraint::Percentage(25),
            Constraint::Percentage(20),
            Constraint::Percentage(10),
        ])
        .split(f.area());

    // Row 1: Devices + Characteristics (side by side)
    let top_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer[0]);

    // Row 2: Details + Read/Write (side by side)
    let mid_row = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer[1]);

    let selected_idx = app
        .table_state
        .selected()
        .unwrap_or(0)
        .min(app.devices.len().saturating_sub(1));
    let selected_device = app.devices.get(selected_idx);

    // Device table (top-left)
    let device_table_widget = device_table(
        app.table_state.selected(),
        &app.devices,
        app.focus == FocusPanel::DeviceList,
    );
    f.render_stateful_widget(device_table_widget, top_row[0], &mut app.table_state);

    // Characteristics panel (top-right)
    let char_panel = characteristic_panel(
        &app.selected_characteristics,
        app.char_table_state.selected(),
        &app.char_values,
        &app.subscribed_chars,
        app.focus == FocusPanel::Characteristics,
    );
    f.render_stateful_widget(char_panel, top_row[1], &mut app.char_table_state);

    // Detail table (mid-left)
    let is_selected_device_connected = app.is_connected
        && selected_device
            .zip(app.connected_device.as_deref())
            .is_some_and(|(sel, conn)| sel.get_id() == conn.get_id());
    let detail_table_widget = detail_table(selected_device, is_selected_device_connected);
    f.render_widget(detail_table_widget, mid_row[0]);

    // Read/Write panel (mid-right)
    let selected_char = app.selected_characteristic();
    let char_uuid = selected_char.map(|c| c.uuid);
    let char_value = char_uuid.and_then(|u| app.char_values.get(&u));
    let is_subscribed = char_uuid.is_some_and(|u| app.subscribed_chars.contains(&u));

    let rw = rw_panel(
        selected_char,
        char_value,
        &app.data_format,
        &app.input_mode,
        &app.input_buffer,
        app.focus == FocusPanel::ReadWrite,
        is_subscribed,
    );
    f.render_widget(rw, mid_row[1]);

    // Message log (bottom area)
    let log = message_log(
        &app.message_log,
        app.log_scroll,
        outer[2].height,
        app.focus == FocusPanel::MessageLog,
    );
    f.render_widget(log, outer[2]);

    // Info bar
    let info = info_table(
        &app.mode,
        &app.input_mode,
        app.is_connected,
        app.pause_status.load(Ordering::SeqCst),
        app.is_loading,
        app.frame_count,
        false,
    );
    f.render_widget(info, outer[3]);
}

#[cfg(feature = "server")]
fn draw_server_mode(f: &mut ratatui::Frame, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Percentage(35),
            Constraint::Percentage(55),
            Constraint::Percentage(10),
        ])
        .split(f.area());

    // Server config panel
    let current_value = app.get_server_char_value();
    let srv = server_panel(
        &app.server_name,
        &app.server_service_uuid,
        &app.server_char_uuid,
        app.is_advertising,
        &app.server_field_focus,
        &app.input_mode,
        &app.input_buffer,
        true,
        &current_value,
        &app.data_format,
    );
    f.render_widget(srv, outer[0]);

    // Message log
    let log = message_log(&app.message_log, app.log_scroll, outer[1].height, false);
    f.render_widget(log, outer[1]);

    // Info bar
    let info = info_table(
        &app.mode,
        &app.input_mode,
        false,
        false,
        app.is_loading,
        app.frame_count,
        app.is_advertising,
    );
    f.render_widget(info, outer[2]);
}

fn handle_editing_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.input_buffer.clear();
            app.cursor_position = 0;
        }
        KeyCode::Enter => {
            match app.mode {
                AppMode::Client => {
                    if let Err(e) = app.write_selected_characteristic() {
                        app.add_log(LogDirection::Error, e);
                    }
                }
                #[cfg(feature = "server")]
                AppMode::Server => {
                    if app.is_advertising {
                        match app.parse_input() {
                            Ok(data) => {
                                app.set_server_char_value(data);
                            }
                            Err(e) => {
                                app.add_log(LogDirection::Error, e);
                            }
                        }
                    } else {
                        let field = app.server_field_focus.clone();
                        if matches!(field, ServerField::ServiceUuid | ServerField::CharUuid)
                            && uuid::Uuid::parse_str(&app.input_buffer).is_err()
                        {
                            app.error_message = format!("Invalid UUID: '{}'", app.input_buffer);
                            app.error_view = true;
                            return;
                        }
                        let value = app.input_buffer.clone();
                        app.set_server_field_value(&field, value);
                    }
                }
            }
            app.input_mode = InputMode::Normal;
            app.input_buffer.clear();
            app.cursor_position = 0;
        }
        KeyCode::Backspace => {
            app.delete_char();
        }
        KeyCode::Char('t')
            if app.input_buffer.is_empty()
                && (app.mode == AppMode::Client || app.is_advertising) =>
        {
            app.toggle_data_format();
        }
        KeyCode::Char(c) => {
            app.insert_char(c);
        }
        KeyCode::Left => {
            app.cursor_left();
        }
        KeyCode::Right => {
            app.cursor_right();
        }
        _ => {}
    }
}

#[allow(clippy::too_many_lines)]
async fn handle_client_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Tab => {
            app.cycle_focus();
        }
        #[cfg(feature = "server")]
        KeyCode::Char('m') => {
            app.stop_scan().await;
            app.toggle_mode();
        }
        KeyCode::Char('s') if !app.is_connected => {
            let current = app.pause_status.load(Ordering::SeqCst);
            app.pause_status.store(!current, Ordering::SeqCst);
        }
        KeyCode::Char('e') if !app.is_connected => {
            app.error_message = match app.get_devices_csv() {
                Ok(msg) => msg,
                Err(e) => e.to_string(),
            };
            app.error_view = true;
        }
        KeyCode::Enter => match app.focus {
            FocusPanel::DeviceList if !app.is_connected => {
                app.connect().await;
            }
            _ => {}
        },
        KeyCode::Char('c') if !app.is_connected => {
            app.connect().await;
        }
        KeyCode::Char('d') if app.is_connected => {
            let selected_idx = app
                .table_state
                .selected()
                .unwrap_or(0)
                .min(app.devices.len().saturating_sub(1));
            let selected = app.devices.get(selected_idx);
            let is_viewing_connected = selected
                .zip(app.connected_device.as_deref())
                .is_some_and(|(sel, conn)| sel.get_id() == conn.get_id());
            if is_viewing_connected {
                app.disconnect();
            }
        }
        KeyCode::Char('r') if app.is_connected => {
            app.read_selected_characteristic();
            app.add_log(
                LogDirection::Info,
                format!(
                    "Reading {}",
                    app.selected_characteristic()
                        .map(|c| c.uuid.to_string())
                        .unwrap_or_default()
                ),
            );
        }
        KeyCode::Char('w') if app.is_connected => {
            app.focus = FocusPanel::ReadWrite;
            app.input_mode = InputMode::Editing;
        }
        KeyCode::Char('i') if app.is_connected && app.focus == FocusPanel::ReadWrite => {
            app.input_mode = InputMode::Editing;
        }
        KeyCode::Char('n') if app.is_connected => {
            let uuid_str = app
                .selected_characteristic()
                .map(|c| c.uuid.to_string())
                .unwrap_or_default();
            let was_subscribed = app
                .selected_characteristic()
                .is_some_and(|c| app.subscribed_chars.contains(&c.uuid));
            app.toggle_subscribe();
            if was_subscribed {
                app.add_log(LogDirection::Info, format!("Unsubscribing from {uuid_str}"));
            } else {
                app.add_log(LogDirection::Info, format!("Subscribing to {uuid_str}"));
            }
        }
        KeyCode::Char('t') if app.focus == FocusPanel::ReadWrite => {
            app.toggle_data_format();
        }
        KeyCode::Down | KeyCode::Char('j') => match app.focus {
            FocusPanel::DeviceList if !app.devices.is_empty() => {
                let next = match app.table_state.selected() {
                    Some(sel) => {
                        if sel >= app.devices.len() - 1 {
                            0
                        } else {
                            sel + 1
                        }
                    }
                    None => 0,
                };
                app.table_state.select(Some(next));
            }
            FocusPanel::Characteristics if !app.selected_characteristics.is_empty() => {
                let next = match app.char_table_state.selected() {
                    Some(sel) => {
                        if sel >= app.selected_characteristics.len() - 1 {
                            0
                        } else {
                            sel + 1
                        }
                    }
                    None => 0,
                };
                app.char_table_state.select(Some(next));
            }
            FocusPanel::MessageLog => {
                let max_scroll = app.message_log.len().saturating_sub(1);
                app.log_scroll = (app.log_scroll + 1).min(max_scroll);
            }
            _ => {}
        },
        KeyCode::Up | KeyCode::Char('k') => match app.focus {
            FocusPanel::DeviceList if !app.devices.is_empty() => {
                let prev = match app.table_state.selected() {
                    Some(sel) => {
                        if sel == 0 {
                            app.devices.len() - 1
                        } else {
                            sel - 1
                        }
                    }
                    None => 0,
                };
                app.table_state.select(Some(prev));
            }
            FocusPanel::Characteristics if !app.selected_characteristics.is_empty() => {
                let prev = match app.char_table_state.selected() {
                    Some(sel) => {
                        if sel == 0 {
                            app.selected_characteristics.len() - 1
                        } else {
                            sel - 1
                        }
                    }
                    None => 0,
                };
                app.char_table_state.select(Some(prev));
            }
            FocusPanel::MessageLog => {
                app.log_scroll = app.log_scroll.saturating_sub(1);
            }
            _ => {}
        },
        _ => {}
    }
}

#[cfg(feature = "server")]
async fn handle_server_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Char('m') => {
            if app.is_advertising {
                app.stop_server().await;
            }
            app.toggle_mode();
            app.scan();
        }
        KeyCode::Char('a') if !app.is_advertising => {
            app.start_server().await;
        }
        KeyCode::Char('x') if app.is_advertising => {
            app.stop_server().await;
        }
        KeyCode::Char('w') if app.is_advertising => {
            app.input_mode = InputMode::Editing;
        }
        KeyCode::Char('n') if app.is_advertising => {
            app.send_server_notify().await;
        }
        KeyCode::Char('t') if app.is_advertising => {
            app.toggle_data_format();
        }
        KeyCode::Enter if !app.is_advertising => {
            let field = &app.server_field_focus;
            let current = app.server_field_value(field).to_string();
            app.input_buffer = current;
            app.cursor_position = app.input_buffer.len();
            app.input_mode = InputMode::Editing;
        }
        KeyCode::Down | KeyCode::Char('j') if !app.is_advertising => {
            app.server_field_focus = app.server_field_focus.next();
        }
        KeyCode::Up | KeyCode::Char('k') if !app.is_advertising => {
            app.server_field_focus = app.server_field_focus.prev();
        }
        KeyCode::Down | KeyCode::Char('j') if app.is_advertising => {
            let max_scroll = app.message_log.len().saturating_sub(1);
            app.log_scroll = (app.log_scroll + 1).min(max_scroll);
        }
        KeyCode::Up | KeyCode::Char('k') if app.is_advertising => {
            app.log_scroll = app.log_scroll.saturating_sub(1);
        }
        _ => {}
    }
}

fn process_channel_messages(app: &mut App) {
    while let Ok(msg) = app.rx.try_recv() {
        match msg {
            DeviceData::DeviceInfo(boxed) => {
                let device = *boxed;
                if let Some(existing) = app
                    .devices
                    .iter_mut()
                    .find(|d| d.get_id() == device.get_id())
                {
                    existing.name = device.name;
                    existing.rssi = device.rssi;
                    existing.tx_power = device.tx_power;
                    existing.manufacturer_data = device.manufacturer_data;
                    existing.services = device.services;
                    existing.service_data = device.service_data;
                    existing.device = device.device;
                } else {
                    app.devices.push(device);
                    if app.devices.len() > MAX_DEVICES {
                        let excess = app.devices.len() - MAX_DEVICES;
                        let connected_id = app.connected_device.as_ref().map(|d| d.get_id());
                        let indices_to_remove: Vec<usize> = (0..excess)
                            .filter(|&i| {
                                connected_id
                                    .as_ref()
                                    .is_none_or(|cid| app.devices[i].get_id() != *cid)
                            })
                            .collect();
                        let removed_before_selected = indices_to_remove
                            .iter()
                            .filter(|&&i| i < app.table_state.selected().unwrap_or(0))
                            .count();
                        for i in indices_to_remove.into_iter().rev() {
                            app.devices.remove(i);
                        }
                        let selected = app.table_state.selected().unwrap_or(0);
                        let new_selected = selected
                            .saturating_sub(removed_before_selected)
                            .min(app.devices.len().saturating_sub(1));
                        app.table_state.select(if app.devices.is_empty() {
                            None
                        } else {
                            Some(new_selected)
                        });
                    }
                }
                if app.table_state.selected().is_none() && !app.devices.is_empty() {
                    app.table_state.select(Some(0));
                }
            }
            DeviceData::Characteristics {
                device_id,
                characteristics,
            } => {
                let matches = app
                    .connected_device
                    .as_ref()
                    .is_some_and(|d| d.get_id() == device_id);
                if matches {
                    app.selected_characteristics = characteristics;
                    app.is_connected = true;
                    app.is_loading = false;
                    if !app.selected_characteristics.is_empty() {
                        app.char_table_state.select(Some(0));
                    }
                    app.add_log(
                        LogDirection::Info,
                        format!(
                            "Connected — {} characteristics discovered",
                            app.selected_characteristics.len()
                        ),
                    );
                }
            }
            DeviceData::CharacteristicValue { uuid, value } => {
                if app.connected_device.is_some() {
                    let hex = bytes_to_hex(&value);
                    app.add_log(LogDirection::Received, format!("{hex} ({uuid})"));
                    app.insert_char_value(uuid, value);
                }
            }
            DeviceData::Notification { uuid, value } => {
                if app.connected_device.is_some() {
                    let hex = bytes_to_hex(&value);
                    app.add_log(LogDirection::Received, format!("{hex} ({uuid})"));
                    app.insert_char_value(uuid, value);
                }
            }
            DeviceData::WriteComplete { uuid } => {
                app.add_log(LogDirection::Info, format!("Write complete ({uuid})"));
            }
            DeviceData::SubscribeComplete { uuid } => {
                if app.connected_device.is_some() {
                    app.subscribed_chars.insert(uuid);
                    app.add_log(
                        LogDirection::Info,
                        format!("Subscribed to notifications ({uuid})"),
                    );
                }
            }
            DeviceData::UnsubscribeComplete { uuid } => {
                if app.connected_device.is_some() {
                    app.subscribed_chars.remove(&uuid);
                    app.add_log(
                        LogDirection::Info,
                        format!("Unsubscribed from notifications ({uuid})"),
                    );
                }
            }
            DeviceData::Error(error) => {
                app.add_log(LogDirection::Error, error.clone());
                app.error_message = error;
                app.error_view = true;
                app.is_loading = false;
                if app.connected_device.is_some() {
                    app.clear_connection_state();
                }
            }
            DeviceData::Info(info) => {
                app.add_log(LogDirection::Info, info);
            }
            #[cfg(feature = "server")]
            DeviceData::ServerLog { direction, message } => {
                app.add_log(direction, message);
            }
        }
    }
}
