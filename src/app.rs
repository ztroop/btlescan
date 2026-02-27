use std::{
    collections::{HashMap, HashSet},
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use ratatui::widgets::TableState;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use uuid::Uuid;

use crate::{
    scan::{
        bluetooth_scan, connect_device, disconnect_device, read_characteristic_value,
        subscribe_to_notifications, unsubscribe_from_notifications, write_characteristic_value,
    },
    server::{self, ServerHandle},
    structs::{
        AppMode, Characteristic, DataFormat, DeviceCsv, DeviceInfo, FocusPanel, InputMode,
        LogDirection, LogEntry, ServerField,
    },
    utils::{bytes_to_hex, hex_to_bytes},
};

pub enum DeviceData {
    DeviceInfo(Box<DeviceInfo>),
    Characteristics(Vec<Characteristic>),
    CharacteristicValue {
        uuid: Uuid,
        value: Vec<u8>,
    },
    Notification {
        uuid: Uuid,
        value: Vec<u8>,
    },
    WriteComplete {
        uuid: Uuid,
    },
    SubscribeComplete {
        uuid: Uuid,
    },
    UnsubscribeComplete {
        uuid: Uuid,
    },
    Error(String),
    Info(String),
    ServerLog {
        direction: LogDirection,
        message: String,
    },
}

pub struct App {
    pub rx: UnboundedReceiver<DeviceData>,
    pub tx: UnboundedSender<DeviceData>,

    #[allow(dead_code)]
    pub loading_status: Arc<AtomicBool>,
    pub pause_status: Arc<AtomicBool>,

    // Mode and navigation
    pub mode: AppMode,
    pub focus: FocusPanel,
    pub input_mode: InputMode,
    pub data_format: DataFormat,

    // Device list
    pub table_state: TableState,
    pub devices: Vec<DeviceInfo>,

    // Connection
    pub connected_device: Option<Arc<DeviceInfo>>,
    pub is_connected: bool,

    // Characteristics
    pub selected_characteristics: Vec<Characteristic>,
    pub char_table_state: TableState,
    pub char_values: HashMap<Uuid, Vec<u8>>,
    pub subscribed_chars: HashSet<Uuid>,

    // Input
    pub input_buffer: String,
    pub cursor_position: usize,

    // Message log
    pub message_log: Vec<LogEntry>,
    pub log_scroll: usize,

    // Server
    pub server_name: String,
    pub server_service_uuid: String,
    pub server_char_uuid: String,
    pub server_field_focus: ServerField,
    pub is_advertising: bool,
    pub server_handle: Option<ServerHandle>,
    pub server_shared_value: Arc<Mutex<Vec<u8>>>,

    // UI state
    pub frame_count: usize,
    pub is_loading: bool,
    pub error_view: bool,
    pub error_message: String,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            tx,
            rx,
            loading_status: Arc::new(AtomicBool::default()),
            pause_status: Arc::new(AtomicBool::default()),

            mode: AppMode::Client,
            focus: FocusPanel::DeviceList,
            input_mode: InputMode::Normal,
            data_format: DataFormat::Hex,

            table_state: TableState::default(),
            devices: Vec::new(),

            connected_device: None,
            is_connected: false,

            selected_characteristics: Vec::new(),
            char_table_state: TableState::default(),
            char_values: HashMap::new(),
            subscribed_chars: HashSet::new(),

            input_buffer: String::new(),
            cursor_position: 0,

            message_log: Vec::new(),
            log_scroll: 0,

            server_name: "btlescan".to_string(),
            server_service_uuid: "0000180d-0000-1000-8000-00805f9b34fb".to_string(),
            server_char_uuid: "00002a37-0000-1000-8000-00805f9b34fb".to_string(),
            server_field_focus: ServerField::Name,
            is_advertising: false,
            server_handle: None,
            server_shared_value: Arc::new(Mutex::new(Vec::new())),

            frame_count: 0,
            is_loading: false,
            error_view: false,
            error_message: String::new(),
            should_quit: false,
        }
    }

    pub async fn scan(&mut self) {
        let pause_signal_clone = Arc::clone(&self.pause_status);
        let tx_clone = self.tx.clone();
        tokio::spawn(async move { bluetooth_scan(tx_clone, pause_signal_clone).await });
    }

    pub async fn connect(&mut self) {
        let selected_device = self
            .devices
            .get(self.table_state.selected().unwrap_or(0))
            .cloned();

        if let Some(device) = selected_device {
            self.pause_status.store(true, Ordering::SeqCst);
            self.is_loading = true;
            let device = Arc::new(device);
            self.connected_device = Some(Arc::clone(&device));
            let tx_clone = self.tx.clone();
            tokio::spawn(async move { connect_device(tx_clone, device).await });
        }
    }

    pub async fn disconnect(&mut self) {
        if let Some(device) = &self.connected_device {
            let device = Arc::clone(device);
            let tx_clone = self.tx.clone();
            tokio::spawn(async move { disconnect_device(tx_clone, device).await });
        }
        self.is_connected = false;
        self.connected_device = None;
        self.selected_characteristics.clear();
        self.char_table_state = TableState::default();
        self.char_values.clear();
        self.subscribed_chars.clear();
        self.input_buffer.clear();
        self.cursor_position = 0;
        self.pause_status.store(false, Ordering::SeqCst);
        self.add_log(LogDirection::Info, "Disconnected from device".into());
    }

    pub fn read_selected_characteristic(&self) {
        let char_opt = self.selected_characteristic();
        let peripheral = self
            .connected_device
            .as_ref()
            .and_then(|d| d.device.clone());

        if let (Some(ch), Some(device)) = (char_opt, peripheral) {
            if let Some(handle) = &ch.handle {
                let tx = self.tx.clone();
                let handle = handle.clone();
                tokio::spawn(async move { read_characteristic_value(tx, device, handle).await });
            }
        }
    }

    pub fn write_selected_characteristic(&mut self) -> Result<(), String> {
        let data = self.parse_input()?;
        let char_opt = self.selected_characteristic().cloned();
        let peripheral = self
            .connected_device
            .as_ref()
            .and_then(|d| d.device.clone());

        if let (Some(ch), Some(device)) = (char_opt, peripheral) {
            if let Some(handle) = ch.handle {
                let hex_str = bytes_to_hex(&data);
                self.add_log(LogDirection::Sent, format!("{} ({})", hex_str, handle.uuid));
                let tx = self.tx.clone();
                let data_clone = data;
                tokio::spawn(async move {
                    write_characteristic_value(tx, device, handle, data_clone).await
                });
                self.input_buffer.clear();
                self.cursor_position = 0;
                Ok(())
            } else {
                Err("No handle for characteristic".into())
            }
        } else {
            Err("No characteristic or device selected".into())
        }
    }

    pub fn toggle_subscribe(&mut self) {
        let char_opt = self.selected_characteristic().cloned();
        let peripheral = self
            .connected_device
            .as_ref()
            .and_then(|d| d.device.clone());

        if let (Some(ch), Some(device)) = (char_opt, peripheral) {
            if let Some(handle) = ch.handle {
                let tx = self.tx.clone();
                if self.subscribed_chars.contains(&ch.uuid) {
                    tokio::spawn(async move {
                        unsubscribe_from_notifications(tx, device, handle).await
                    });
                } else {
                    tokio::spawn(
                        async move { subscribe_to_notifications(tx, device, handle).await },
                    );
                }
            }
        }
    }

    pub fn selected_characteristic(&self) -> Option<&Characteristic> {
        let idx = self.char_table_state.selected()?;
        self.selected_characteristics.get(idx)
    }

    pub fn add_log(&mut self, direction: LogDirection, message: String) {
        self.message_log.push(LogEntry::new(direction, message));
        self.log_scroll = self.message_log.len().saturating_sub(1);
    }

    pub fn cycle_focus(&mut self) {
        self.focus = self.focus.next();
    }

    pub fn toggle_data_format(&mut self) {
        self.data_format = self.data_format.toggle();
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            AppMode::Client => {
                self.pause_status.store(true, Ordering::SeqCst);
                AppMode::Server
            }
            AppMode::Server => AppMode::Client,
        };
    }

    pub async fn start_server(&mut self) {
        if self.is_advertising {
            return;
        }
        let service_uuid = match Uuid::parse_str(&self.server_service_uuid) {
            Ok(u) => u,
            Err(e) => {
                self.error_message = format!("Invalid service UUID: {}", e);
                self.error_view = true;
                return;
            }
        };
        let char_uuid = match Uuid::parse_str(&self.server_char_uuid) {
            Ok(u) => u,
            Err(e) => {
                self.error_message = format!("Invalid characteristic UUID: {}", e);
                self.error_view = true;
                return;
            }
        };
        let tx = self.tx.clone();
        let name = self.server_name.clone();
        let shared_value = Arc::clone(&self.server_shared_value);
        match server::start_server(tx, name, service_uuid, char_uuid, shared_value).await {
            Ok(handle) => {
                self.server_handle = Some(handle);
                self.is_advertising = true;
                self.add_log(LogDirection::Info, "GATT server advertising started".into());
            }
            Err(e) => {
                self.add_log(LogDirection::Error, format!("Server start failed: {}", e));
                self.error_message = format!("Server start failed: {}", e);
                self.error_view = true;
            }
        }
    }

    pub async fn stop_server(&mut self) {
        if let Some(mut handle) = self.server_handle.take() {
            handle.stop().await;
        }
        self.is_advertising = false;
        *self.server_shared_value.lock().unwrap() = Vec::new();
        self.add_log(LogDirection::Info, "GATT server stopped".into());
    }

    pub fn set_server_char_value(&mut self, data: Vec<u8>) {
        let hex_str = bytes_to_hex(&data);
        if let Some(handle) = &self.server_handle {
            handle.set_value(data);
            self.add_log(LogDirection::Info, format!("Value set: {}", hex_str));
        }
    }

    pub async fn send_server_notify(&mut self) {
        if let Some(handle) = &mut self.server_handle {
            let value = handle.get_value();
            if value.is_empty() {
                self.add_log(
                    LogDirection::Error,
                    "No value set — use 'w' to set a value first".into(),
                );
                return;
            }
            let hex_str = bytes_to_hex(&value);
            match handle.update_value(value).await {
                Ok(_) => {
                    self.add_log(LogDirection::Sent, format!("Notify: {}", hex_str));
                }
                Err(e) => {
                    self.add_log(LogDirection::Error, format!("Notify failed: {}", e));
                }
            }
        }
    }

    pub fn get_server_char_value(&self) -> Vec<u8> {
        self.server_shared_value.lock().unwrap().clone()
    }

    pub fn server_field_value(&self, field: &ServerField) -> &str {
        match field {
            ServerField::Name => &self.server_name,
            ServerField::ServiceUuid => &self.server_service_uuid,
            ServerField::CharUuid => &self.server_char_uuid,
        }
    }

    pub fn set_server_field_value(&mut self, field: &ServerField, value: String) {
        match field {
            ServerField::Name => self.server_name = value,
            ServerField::ServiceUuid => self.server_service_uuid = value,
            ServerField::CharUuid => self.server_char_uuid = value,
        }
    }

    pub fn parse_input(&self) -> Result<Vec<u8>, String> {
        match self.data_format {
            DataFormat::Hex => hex_to_bytes(&self.input_buffer),
            DataFormat::Text => Ok(self.input_buffer.as_bytes().to_vec()),
        }
    }

    pub fn insert_char(&mut self, c: char) {
        self.input_buffer.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            let prev = self.input_buffer[..self.cursor_position]
                .chars()
                .last()
                .map_or(0, |c| c.len_utf8());
            self.cursor_position -= prev;
            self.input_buffer.remove(self.cursor_position);
        }
    }

    pub fn get_devices_csv(&self) -> Result<String, Box<dyn Error>> {
        let now = chrono::Local::now();
        let timestamp = now.format("%Y-%m-%d_%H-%M-%S").to_string();
        let file_path = format!("btlescan_{}.csv", timestamp);
        let file = std::fs::File::create(file_path).expect("Unable to create file");
        let mut wtr = csv::Writer::from_writer(file);
        for device in &self.devices {
            wtr.serialize(DeviceCsv {
                id: device.id.clone(),
                name: device.name.clone(),
                tx_power: device.tx_power.clone(),
                address: device.address.clone(),
                rssi: device.rssi.clone(),
            })?;
        }
        wtr.flush()?;
        Ok("Devices exported to a CSV file in the current directory.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new_defaults() {
        let app = App::new();
        assert_eq!(app.mode, AppMode::Client);
        assert_eq!(app.focus, FocusPanel::DeviceList);
        assert_eq!(app.input_mode, InputMode::Normal);
        assert_eq!(app.data_format, DataFormat::Hex);
        assert!(!app.is_connected);
        assert!(app.devices.is_empty());
        assert!(app.selected_characteristics.is_empty());
        assert!(app.message_log.is_empty());
        assert_eq!(app.server_name, "btlescan");
    }

    #[test]
    fn test_cycle_focus() {
        let mut app = App::new();
        assert_eq!(app.focus, FocusPanel::DeviceList);
        app.cycle_focus();
        assert_eq!(app.focus, FocusPanel::Characteristics);
        app.cycle_focus();
        assert_eq!(app.focus, FocusPanel::ReadWrite);
        app.cycle_focus();
        assert_eq!(app.focus, FocusPanel::MessageLog);
        app.cycle_focus();
        assert_eq!(app.focus, FocusPanel::DeviceList);
    }

    #[test]
    fn test_toggle_mode() {
        let mut app = App::new();
        assert_eq!(app.mode, AppMode::Client);
        app.toggle_mode();
        assert_eq!(app.mode, AppMode::Server);
        app.toggle_mode();
        assert_eq!(app.mode, AppMode::Client);
    }

    #[test]
    fn test_toggle_data_format() {
        let mut app = App::new();
        assert_eq!(app.data_format, DataFormat::Hex);
        app.toggle_data_format();
        assert_eq!(app.data_format, DataFormat::Text);
        app.toggle_data_format();
        assert_eq!(app.data_format, DataFormat::Hex);
    }

    #[test]
    fn test_add_log() {
        let mut app = App::new();
        app.add_log(LogDirection::Info, "Test message".into());
        assert_eq!(app.message_log.len(), 1);
        assert_eq!(app.message_log[0].message, "Test message");
        assert_eq!(app.message_log[0].direction, LogDirection::Info);
    }

    #[test]
    fn test_insert_and_delete_char() {
        let mut app = App::new();
        app.insert_char('A');
        app.insert_char('B');
        app.insert_char('C');
        assert_eq!(app.input_buffer, "ABC");
        assert_eq!(app.cursor_position, 3);

        app.delete_char();
        assert_eq!(app.input_buffer, "AB");
        assert_eq!(app.cursor_position, 2);

        app.delete_char();
        app.delete_char();
        assert_eq!(app.input_buffer, "");
        assert_eq!(app.cursor_position, 0);

        // Deleting from empty should not panic
        app.delete_char();
        assert_eq!(app.input_buffer, "");
    }

    #[test]
    fn test_parse_input_hex() {
        let mut app = App::new();
        app.data_format = DataFormat::Hex;
        app.input_buffer = "00 50".to_string();
        assert_eq!(app.parse_input().unwrap(), vec![0x00, 0x50]);
    }

    #[test]
    fn test_parse_input_text() {
        let mut app = App::new();
        app.data_format = DataFormat::Text;
        app.input_buffer = "Hi".to_string();
        assert_eq!(app.parse_input().unwrap(), vec![0x48, 0x69]);
    }

    #[test]
    fn test_parse_input_invalid_hex() {
        let mut app = App::new();
        app.data_format = DataFormat::Hex;
        app.input_buffer = "ZZ".to_string();
        assert!(app.parse_input().is_err());
    }

    #[test]
    fn test_selected_characteristic_none() {
        let app = App::new();
        assert!(app.selected_characteristic().is_none());
    }

    #[test]
    fn test_log_scroll_follows_latest() {
        let mut app = App::new();
        for i in 0..10 {
            app.add_log(LogDirection::Info, format!("msg {}", i));
        }
        assert_eq!(app.log_scroll, 9);
    }

    #[test]
    fn test_server_field_defaults() {
        let app = App::new();
        assert_eq!(app.server_name, "btlescan");
        assert_eq!(
            app.server_service_uuid,
            "0000180d-0000-1000-8000-00805f9b34fb"
        );
        assert_eq!(app.server_char_uuid, "00002a37-0000-1000-8000-00805f9b34fb");
        assert_eq!(app.server_field_focus, ServerField::Name);
    }

    #[test]
    fn test_server_field_get_set() {
        let mut app = App::new();
        assert_eq!(app.server_field_value(&ServerField::Name), "btlescan");

        app.set_server_field_value(&ServerField::Name, "my-device".into());
        assert_eq!(app.server_field_value(&ServerField::Name), "my-device");

        app.set_server_field_value(
            &ServerField::ServiceUuid,
            "b42e2a68-ade7-11e4-89d3-123b93f75cba".into(),
        );
        assert_eq!(
            app.server_field_value(&ServerField::ServiceUuid),
            "b42e2a68-ade7-11e4-89d3-123b93f75cba"
        );
    }

    #[test]
    fn test_server_uuid_validation() {
        assert!(Uuid::parse_str("0000180d-0000-1000-8000-00805f9b34fb").is_ok());
        assert!(Uuid::parse_str("b42e2a68-ade7-11e4-89d3-123b93f75cba").is_ok());
        assert!(Uuid::parse_str("not-a-uuid").is_err());
        assert!(Uuid::parse_str("").is_err());
    }
}
