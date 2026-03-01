use std::{
    collections::{HashMap, HashSet},
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

#[cfg(feature = "server")]
use parking_lot::Mutex;

use ratatui::widgets::TableState;
use tokio::sync::{
    mpsc::{self, UnboundedReceiver, UnboundedSender},
    oneshot,
};
use uuid::Uuid;

use crate::{
    scan::{
        bluetooth_scan, connect_device, disconnect_device, read_characteristic_value,
        subscribe_to_notifications, unsubscribe_from_notifications, write_characteristic_value,
    },
    structs::{
        AppMode, Characteristic, DataFormat, DeviceCsv, DeviceInfo, FocusPanel, InputMode,
        LogDirection, LogEntry,
    },
    utils::{bytes_to_hex, hex_to_bytes},
};

#[cfg(feature = "server")]
use crate::{
    server::{self, ServerHandle},
    structs::ServerField,
};

pub enum DeviceData {
    DeviceInfo(Box<DeviceInfo>),
    Characteristics {
        device_id: String,
        characteristics: Vec<Characteristic>,
    },
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
    #[cfg(feature = "server")]
    ServerLog {
        direction: LogDirection,
        message: String,
    },
}

/// Sends to the app channel; logs if receiver was dropped (e.g. during shutdown).
pub(crate) fn send_or_log(tx: &mpsc::UnboundedSender<DeviceData>, data: DeviceData) {
    if tx.send(data).is_err() {
        eprintln!("btlescan: channel send failed (receiver dropped)");
    }
}

/// Maximum number of log entries to retain. Smaller in tests for faster verification.
#[cfg(test)]
const MAX_LOG_ENTRIES: usize = 10;
#[cfg(not(test))]
const MAX_LOG_ENTRIES: usize = 1000;

/// Maximum number of characteristic values to retain. Prevents unbounded memory growth with many UUIDs.
const MAX_CHAR_VALUES: usize = 100;

/// Maximum number of discovered devices to retain. Prevents unbounded memory growth in busy environments.
pub(crate) const MAX_DEVICES: usize = 500;

/// Maximum input buffer length (characters). Prevents unbounded memory growth from paste/typing.
const MAX_INPUT_LEN: usize = 16 * 1024;

#[allow(clippy::struct_excessive_bools)]
pub struct App {
    pub rx: UnboundedReceiver<DeviceData>,
    pub tx: UnboundedSender<DeviceData>,

    pub pause_status: Arc<AtomicBool>,
    pub scan_shutdown: Option<oneshot::Sender<()>>,
    pub scan_handle: Option<tokio::task::JoinHandle<()>>,

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
    /// Insertion order for char_values; used to evict oldest when exceeding MAX_CHAR_VALUES.
    char_values_order: Vec<Uuid>,
    pub subscribed_chars: HashSet<Uuid>,

    // Input
    pub input_buffer: String,
    pub cursor_position: usize,

    // Message log
    pub message_log: Vec<LogEntry>,
    pub log_scroll: usize,

    // Server
    pub is_advertising: bool,
    #[cfg(feature = "server")]
    pub server_name: String,
    #[cfg(feature = "server")]
    pub server_service_uuid: String,
    #[cfg(feature = "server")]
    pub server_char_uuid: String,
    #[cfg(feature = "server")]
    pub server_field_focus: ServerField,
    #[cfg(feature = "server")]
    pub server_handle: Option<ServerHandle>,
    #[cfg(feature = "server")]
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
            pause_status: Arc::new(AtomicBool::default()),
            scan_shutdown: None,
            scan_handle: None,

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
            char_values_order: Vec::new(),
            subscribed_chars: HashSet::new(),

            input_buffer: String::new(),
            cursor_position: 0,

            message_log: Vec::new(),
            log_scroll: 0,

            is_advertising: false,
            #[cfg(feature = "server")]
            server_name: "btlescan".to_string(),
            #[cfg(feature = "server")]
            server_service_uuid: "0000180d-0000-1000-8000-00805f9b34fb".to_string(),
            #[cfg(feature = "server")]
            server_char_uuid: "00002a37-0000-1000-8000-00805f9b34fb".to_string(),
            #[cfg(feature = "server")]
            server_field_focus: ServerField::Name,
            #[cfg(feature = "server")]
            server_handle: None,
            #[cfg(feature = "server")]
            server_shared_value: Arc::new(Mutex::new(Vec::new())),

            frame_count: 0,
            is_loading: false,
            error_view: false,
            error_message: String::new(),
            should_quit: false,
        }
    }

    pub fn scan(&mut self) {
        self.pause_status.store(false, Ordering::SeqCst);
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        self.scan_shutdown = Some(shutdown_tx);
        let pause_signal_clone = Arc::clone(&self.pause_status);
        let tx_clone = self.tx.clone();
        self.scan_handle = Some(tokio::spawn(async move {
            bluetooth_scan(tx_clone, pause_signal_clone, shutdown_rx).await;
        }));
    }

    pub async fn stop_scan(&mut self) {
        if let Some(tx) = self.scan_shutdown.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.scan_handle.take() {
            let _ = handle.await;
        }
        self.pause_status.store(true, Ordering::SeqCst);
    }

    pub async fn connect(&mut self) {
        if self.is_loading {
            return;
        }
        let selected_idx = self
            .table_state
            .selected()
            .unwrap_or(0)
            .min(self.devices.len().saturating_sub(1));
        let selected_device = self.devices.get(selected_idx).cloned();

        if let Some(device) = selected_device {
            self.stop_scan().await;
            self.char_values.clear();
            self.char_values_order.clear();
            self.is_loading = true;
            let device = Arc::new(device);
            self.connected_device = Some(Arc::clone(&device));
            let tx_clone = self.tx.clone();
            tokio::spawn(async move { connect_device(tx_clone, device).await });
        } else {
            self.error_message = "No device selected".to_string();
            self.error_view = true;
        }
    }

    /// Disconnects from the device. Returns a JoinHandle when a disconnect task was spawned,
    /// which the caller may await for a clean shutdown (e.g. on quit).
    pub fn disconnect(&mut self) -> Option<tokio::task::JoinHandle<()>> {
        let handle = if let Some(device) = &self.connected_device {
            let device = Arc::clone(device);
            let tx_clone = self.tx.clone();
            Some(tokio::spawn(async move {
                disconnect_device(tx_clone, device).await
            }))
        } else {
            None
        };
        self.is_connected = false;
        self.is_loading = false;
        self.connected_device = None;
        self.selected_characteristics.clear();
        self.char_table_state = TableState::default();
        self.char_values.clear();
        self.char_values_order.clear();
        self.subscribed_chars.clear();
        self.input_buffer.clear();
        self.cursor_position = 0;
        self.add_log(LogDirection::Info, "Disconnected from device".into());
        self.scan();
        handle
    }

    /// Clears connection state (used when Error is received while connected).
    pub fn clear_connection_state(&mut self) {
        self.connected_device = None;
        self.is_connected = false;
        self.selected_characteristics.clear();
        self.char_table_state = TableState::default();
        self.char_values.clear();
        self.char_values_order.clear();
        self.subscribed_chars.clear();
    }

    /// Inserts a characteristic value, evicting oldest entries when exceeding MAX_CHAR_VALUES.
    pub fn insert_char_value(&mut self, uuid: Uuid, value: Vec<u8>) {
        if self.char_values.contains_key(&uuid) {
            self.char_values_order.retain(|u| *u != uuid);
        }
        self.char_values.insert(uuid, value);
        self.char_values_order.push(uuid);
        while self.char_values.len() > MAX_CHAR_VALUES {
            if let Some(oldest) = self.char_values_order.first().copied() {
                self.char_values_order.remove(0);
                self.char_values.remove(&oldest);
            } else {
                break;
            }
        }
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
                self.add_log(LogDirection::Sent, format!("{hex_str} ({})", handle.uuid));
                let tx = self.tx.clone();
                let data_clone = data;
                tokio::spawn(async move {
                    write_characteristic_value(tx, device, handle, data_clone).await;
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
                        unsubscribe_from_notifications(tx, device, handle).await;
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
        if self.message_log.len() > MAX_LOG_ENTRIES {
            self.message_log
                .drain(0..(self.message_log.len() - MAX_LOG_ENTRIES));
        }
        self.log_scroll = self.message_log.len().saturating_sub(1);
    }

    pub fn cycle_focus(&mut self) {
        self.focus = self.focus.next();
    }

    pub fn toggle_data_format(&mut self) {
        self.data_format = self.data_format.toggle();
    }

    #[cfg(feature = "server")]
    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            AppMode::Client => AppMode::Server,
            AppMode::Server => AppMode::Client,
        };
    }

    #[cfg(feature = "server")]
    pub async fn start_server(&mut self) {
        if self.is_advertising {
            return;
        }
        let service_uuid = match Uuid::parse_str(&self.server_service_uuid) {
            Ok(u) => u,
            Err(e) => {
                self.error_message = format!("Invalid service UUID: {e}");
                self.error_view = true;
                return;
            }
        };
        let char_uuid = match Uuid::parse_str(&self.server_char_uuid) {
            Ok(u) => u,
            Err(e) => {
                self.error_message = format!("Invalid characteristic UUID: {e}");
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
                self.add_log(LogDirection::Error, format!("Server start failed: {e}"));
                self.error_message = format!("Server start failed: {e}");
                self.error_view = true;
            }
        }
    }

    #[cfg(feature = "server")]
    pub async fn stop_server(&mut self) {
        if let Some(mut handle) = self.server_handle.take() {
            handle.stop().await;
        }
        self.is_advertising = false;
        *self.server_shared_value.lock() = Vec::new();
        self.add_log(LogDirection::Info, "GATT server stopped".into());
    }

    #[cfg(feature = "server")]
    pub fn set_server_char_value(&mut self, data: Vec<u8>) {
        const MAX_CHARACTERISTIC_SIZE: usize = 512;
        let data = if data.len() > MAX_CHARACTERISTIC_SIZE {
            data.into_iter().take(MAX_CHARACTERISTIC_SIZE).collect()
        } else {
            data
        };
        let hex_str = bytes_to_hex(&data);
        if let Some(handle) = &self.server_handle {
            handle.set_value(data);
            self.add_log(LogDirection::Info, format!("Value set: {hex_str}"));
        }
    }

    #[cfg(feature = "server")]
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
                Ok(()) => {
                    self.add_log(LogDirection::Sent, format!("Notify: {hex_str}"));
                }
                Err(e) => {
                    self.add_log(LogDirection::Error, format!("Notify failed: {e}"));
                }
            }
        }
    }

    #[cfg(feature = "server")]
    pub fn get_server_char_value(&self) -> Vec<u8> {
        self.server_shared_value.lock().clone()
    }

    #[cfg(feature = "server")]
    pub fn server_field_value(&self, field: &ServerField) -> &str {
        match field {
            ServerField::Name => &self.server_name,
            ServerField::ServiceUuid => &self.server_service_uuid,
            ServerField::CharUuid => &self.server_char_uuid,
        }
    }

    #[cfg(feature = "server")]
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
            DataFormat::Text => {
                if self.input_buffer.len() > MAX_INPUT_LEN {
                    Err(format!(
                        "Input exceeds maximum length of {} characters",
                        MAX_INPUT_LEN
                    ))
                } else {
                    Ok(self.input_buffer.as_bytes().to_vec())
                }
            }
        }
    }

    pub fn insert_char(&mut self, c: char) {
        if self.input_buffer.len() >= MAX_INPUT_LEN {
            return;
        }
        self.input_buffer.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            let prev = self.input_buffer[..self.cursor_position]
                .chars()
                .last()
                .map_or(0, char::len_utf8);
            self.cursor_position -= prev;
            self.input_buffer.remove(self.cursor_position);
        }
    }

    /// Move cursor left by one character (respects UTF-8 boundaries).
    pub fn cursor_left(&mut self) {
        if self.cursor_position > 0 {
            let prev = self.input_buffer[..self.cursor_position]
                .chars()
                .last()
                .map_or(0, char::len_utf8);
            self.cursor_position -= prev;
        }
    }

    /// Move cursor right by one character (respects UTF-8 boundaries).
    pub fn cursor_right(&mut self) {
        if self.cursor_position < self.input_buffer.len() {
            let next = self.input_buffer[self.cursor_position..]
                .chars()
                .next()
                .map_or(1, char::len_utf8);
            self.cursor_position += next;
        }
    }

    pub fn get_devices_csv(&self) -> Result<String, Box<dyn Error>> {
        let now = chrono::Local::now();
        let timestamp = now.format("%Y-%m-%d_%H-%M-%S").to_string();
        let file_path = format!("btlescan_{timestamp}.csv");
        let file = std::fs::File::create(&file_path)?;
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
        let full_path = std::path::Path::new(&file_path)
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(&file_path));
        Ok(format!("Devices exported to {}", full_path.display()))
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
        #[cfg(feature = "server")]
        assert_eq!(app.server_name, "btlescan");
    }

    #[tokio::test]
    async fn test_connect_no_device_selected_shows_error() {
        let mut app = App::new();
        app.devices.clear();
        app.connect().await;
        assert_eq!(app.error_message, "No device selected");
        assert!(app.error_view);
    }

    #[tokio::test]
    async fn test_scan_resets_pause_status() {
        let mut app = App::new();
        app.pause_status.store(true, Ordering::SeqCst);
        assert!(app.pause_status.load(Ordering::SeqCst));
        app.scan(); // spawns tokio task, requires runtime
        assert!(!app.pause_status.load(Ordering::SeqCst));
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

    #[cfg(feature = "server")]
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
    fn test_add_log_truncates_when_over_limit() {
        let mut app = App::new();
        for i in 0..15 {
            app.add_log(LogDirection::Info, format!("msg {i}"));
        }
        assert_eq!(app.message_log.len(), 10);
        assert_eq!(app.message_log[0].message, "msg 5");
        assert_eq!(app.message_log[9].message, "msg 14");
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
    fn test_insert_char_caps_at_max() {
        let mut app = App::new();
        for _ in 0..(MAX_INPUT_LEN + 10) {
            app.insert_char('x');
        }
        assert_eq!(app.input_buffer.len(), MAX_INPUT_LEN);
    }

    #[test]
    fn test_parse_input_text_exceeds_max() {
        let mut app = App::new();
        app.data_format = DataFormat::Text;
        app.input_buffer = "x".repeat(MAX_INPUT_LEN + 1);
        assert!(app.parse_input().is_err());
    }

    #[test]
    fn test_selected_characteristic_none() {
        let app = App::new();
        assert!(app.selected_characteristic().is_none());
    }

    #[test]
    fn test_get_devices_csv_returns_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();
        let _guard = scopeguard::guard((), |()| {
            let _ = std::env::set_current_dir(&original_cwd);
        });

        let app = App::new();
        let result = app.get_devices_csv().unwrap();

        assert!(result.starts_with("Devices exported to "));
        assert!(result.contains("btlescan_"));
        assert!(result.contains(".csv"));
    }

    #[test]
    fn test_log_scroll_follows_latest() {
        let mut app = App::new();
        for i in 0..10 {
            app.add_log(LogDirection::Info, format!("msg {i}"));
        }
        assert_eq!(app.log_scroll, 9);
    }

    #[cfg(feature = "server")]
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

    #[cfg(feature = "server")]
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
    fn test_insert_char_value_caps_at_max() {
        let mut app = App::new();
        for i in 0..=MAX_CHAR_VALUES {
            let uuid = Uuid::parse_str(&format!("00000000-0000-1000-8000-{:012x}", i)).unwrap();
            app.insert_char_value(uuid, vec![i as u8]);
        }
        assert_eq!(app.char_values.len(), MAX_CHAR_VALUES);
        assert_eq!(app.char_values_order.len(), MAX_CHAR_VALUES);
        // Oldest (uuid 0) should have been evicted
        let uuid_0 = Uuid::parse_str("00000000-0000-1000-8000-000000000000").unwrap();
        assert!(!app.char_values.contains_key(&uuid_0));
        // Newest (uuid MAX_CHAR_VALUES) should be present
        let uuid_newest =
            Uuid::parse_str(&format!("00000000-0000-1000-8000-{:012x}", MAX_CHAR_VALUES)).unwrap();
        assert_eq!(
            app.char_values.get(&uuid_newest),
            Some(&vec![MAX_CHAR_VALUES as u8])
        );
    }

    #[cfg(feature = "server")]
    #[test]
    fn test_server_uuid_validation() {
        assert!(Uuid::parse_str("0000180d-0000-1000-8000-00805f9b34fb").is_ok());
        assert!(Uuid::parse_str("b42e2a68-ade7-11e4-89d3-123b93f75cba").is_ok());
        assert!(Uuid::parse_str("not-a-uuid").is_err());
        assert!(Uuid::parse_str("").is_err());
    }
}
