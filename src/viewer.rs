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

use crate::app::{App, DeviceData};
use crate::structs::{AppMode, DeviceInfo, FocusPanel, InputMode, LogDirection, ServerField};
use crate::utils::{bytes_to_hex, centered_rect};
use crate::widgets::characteristic_panel::characteristic_panel;
use crate::widgets::detail_table::detail_table;
use crate::widgets::device_table::device_table;
use crate::widgets::info_table::info_table;
use crate::widgets::message_log::message_log;
use crate::widgets::rw_panel::rw_panel;
use crate::widgets::server_panel::server_panel;

pub async fn viewer<B: Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), Box<dyn Error>>
where
    <B as Backend>::Error: 'static,
{
    app.table_state.select(Some(0));

    loop {
        terminal.draw(|f| {
            app.frame_count = f.count();

            match app.mode {
                AppMode::Client => draw_client_mode(f, app),
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
            if let Event::Key(key) = event::read()? {
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
                    AppMode::Server => handle_server_input(app, key.code).await,
                }

                if app.should_quit {
                    return Ok(());
                }
            }
        }

        process_channel_messages(app);
    }
}

fn draw_client_mode(f: &mut ratatui::Frame, app: &mut App) {
    app.frame_count += 1;

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

    let device_binding = &DeviceInfo::default();
    let selected_device = app
        .devices
        .get(app.table_state.selected().unwrap_or(0))
        .unwrap_or(device_binding);

    // Device table (top-left)
    let dev_table = device_table(
        app.table_state.selected(),
        &app.devices,
        app.focus == FocusPanel::DeviceList,
    );
    f.render_stateful_widget(dev_table, top_row[0], &mut app.table_state);

    // Characteristics panel (top-right)
    let char_panel = characteristic_panel(
        &app.selected_characteristics,
        app.char_table_state.selected(),
        &app.char_values,
        &app.subscribed_chars,
        app.focus == FocusPanel::Characteristics,
    );
    f.render_widget(char_panel, top_row[1]);

    // Detail table (mid-left)
    let is_selected_device_connected = app.is_connected
        && app
            .connected_device
            .as_ref()
            .map(|d| d.get_id() == selected_device.get_id())
            .unwrap_or(false);
    let det_table = detail_table(selected_device, is_selected_device_connected);
    f.render_widget(det_table, mid_row[0]);

    // Read/Write panel (mid-right)
    let selected_char = app.selected_characteristic();
    let char_uuid = selected_char.map(|c| c.uuid);
    let char_value = char_uuid.and_then(|u| app.char_values.get(&u));
    let is_subscribed = char_uuid
        .map(|u| app.subscribed_chars.contains(&u))
        .unwrap_or(false);

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
        &app.is_loading,
        &app.frame_count,
    );
    f.render_widget(info, outer[3]);
}

fn draw_server_mode(f: &mut ratatui::Frame, app: &mut App) {
    app.frame_count += 1;

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
    let srv = server_panel(
        &app.server_name,
        &app.server_service_uuid,
        &app.server_char_uuid,
        app.is_advertising,
        &app.server_field_focus,
        &app.input_mode,
        &app.input_buffer,
        true,
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
        &app.is_loading,
        &app.frame_count,
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
                AppMode::Server => {
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
            app.input_mode = InputMode::Normal;
            app.input_buffer.clear();
            app.cursor_position = 0;
        }
        KeyCode::Backspace => {
            app.delete_char();
        }
        KeyCode::Char('t') if app.input_buffer.is_empty() && app.mode == AppMode::Client => {
            app.toggle_data_format();
        }
        KeyCode::Char(c) => {
            app.insert_char(c);
        }
        KeyCode::Left => {
            if app.cursor_position > 0 {
                app.cursor_position -= 1;
            }
        }
        KeyCode::Right => {
            if app.cursor_position < app.input_buffer.len() {
                app.cursor_position += 1;
            }
        }
        _ => {}
    }
}

async fn handle_client_input(app: &mut App, key: KeyCode) {
    match key {
        KeyCode::Char('q') => {
            app.should_quit = true;
        }
        KeyCode::Tab => {
            app.cycle_focus();
        }
        KeyCode::Char('m') => {
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
            let selected = app.devices.get(app.table_state.selected().unwrap_or(0));
            let is_viewing_connected = selected
                .zip(app.connected_device.as_deref())
                .map(|(sel, conn)| sel.get_id() == conn.get_id())
                .unwrap_or(false);
            if is_viewing_connected {
                app.disconnect().await;
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
                .map(|c| app.subscribed_chars.contains(&c.uuid))
                .unwrap_or(false);
            app.toggle_subscribe();
            if was_subscribed {
                app.add_log(
                    LogDirection::Info,
                    format!("Unsubscribing from {}", uuid_str),
                );
            } else {
                app.add_log(LogDirection::Info, format!("Subscribing to {}", uuid_str));
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
        }
        KeyCode::Char('a') if !app.is_advertising => {
            app.start_server().await;
        }
        KeyCode::Char('x') if app.is_advertising => {
            app.stop_server().await;
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
        _ => {}
    }
}

fn process_channel_messages(app: &mut App) {
    while let Ok(msg) = app.rx.try_recv() {
        match msg {
            DeviceData::DeviceInfo(boxed) => {
                let device = *boxed;
                app.devices.push(device);
                if app.table_state.selected().is_none() {
                    app.table_state.select(Some(0));
                }
            }
            DeviceData::Characteristics(characteristics) => {
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
            DeviceData::CharacteristicValue { uuid, value } => {
                let hex = bytes_to_hex(&value);
                app.add_log(LogDirection::Received, format!("{} ({})", hex, uuid));
                app.char_values.insert(uuid, value);
            }
            DeviceData::Notification { uuid, value } => {
                let hex = bytes_to_hex(&value);
                app.add_log(LogDirection::Received, format!("{} ({})", hex, uuid));
                app.char_values.insert(uuid, value);
            }
            DeviceData::WriteComplete { uuid } => {
                app.add_log(LogDirection::Info, format!("Write complete ({})", uuid));
            }
            DeviceData::SubscribeComplete { uuid } => {
                app.subscribed_chars.insert(uuid);
                app.add_log(
                    LogDirection::Info,
                    format!("Subscribed to notifications ({})", uuid),
                );
            }
            DeviceData::UnsubscribeComplete { uuid } => {
                app.subscribed_chars.remove(&uuid);
                app.add_log(
                    LogDirection::Info,
                    format!("Unsubscribed from notifications ({})", uuid),
                );
            }
            DeviceData::Error(error) => {
                app.add_log(LogDirection::Error, error.clone());
                app.error_message = error;
                app.error_view = true;
                app.is_loading = false;
            }
            DeviceData::Info(info) => {
                app.add_log(LogDirection::Info, info);
            }
            DeviceData::ServerLog { direction, message } => {
                app.add_log(direction, message);
            }
        }
    }
}
