use std::collections::HashMap;

use btleplug::api::CharPropFlags;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq)]
pub enum AppMode {
    Client,
    Server,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FocusPanel {
    DeviceList,
    Characteristics,
    ReadWrite,
    MessageLog,
}

impl FocusPanel {
    pub fn next(&self) -> Self {
        match self {
            FocusPanel::DeviceList => FocusPanel::Characteristics,
            FocusPanel::Characteristics => FocusPanel::ReadWrite,
            FocusPanel::ReadWrite => FocusPanel::MessageLog,
            FocusPanel::MessageLog => FocusPanel::DeviceList,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DataFormat {
    Text,
    Hex,
}

impl DataFormat {
    pub fn toggle(&self) -> Self {
        match self {
            DataFormat::Text => DataFormat::Hex,
            DataFormat::Hex => DataFormat::Text,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            DataFormat::Text => "Text",
            DataFormat::Hex => "Hex",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum LogDirection {
    Sent,
    Received,
    Info,
    Error,
}

impl LogDirection {
    pub fn symbol(&self) -> &str {
        match self {
            LogDirection::Sent => "→",
            LogDirection::Received => "←",
            LogDirection::Info => "ℹ",
            LogDirection::Error => "✗",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub timestamp: String,
    pub direction: LogDirection,
    pub message: String,
}

impl LogEntry {
    pub fn new(direction: LogDirection, message: String) -> Self {
        Self {
            timestamp: chrono::Local::now().format("%H:%M:%S.%3f").to_string(),
            direction,
            message,
        }
    }

    #[cfg(test)]
    pub fn with_timestamp(timestamp: &str, direction: LogDirection, message: String) -> Self {
        Self {
            timestamp: timestamp.to_string(),
            direction,
            message,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ServerField {
    Name,
    ServiceUuid,
    CharUuid,
}

impl ServerField {
    pub fn next(&self) -> Self {
        match self {
            ServerField::Name => ServerField::ServiceUuid,
            ServerField::ServiceUuid => ServerField::CharUuid,
            ServerField::CharUuid => ServerField::Name,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            ServerField::Name => ServerField::CharUuid,
            ServerField::ServiceUuid => ServerField::Name,
            ServerField::CharUuid => ServerField::ServiceUuid,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            ServerField::Name => "Device Name",
            ServerField::ServiceUuid => "Service UUID",
            ServerField::CharUuid => "Char UUID",
        }
    }
}

/// A struct to hold the information of a Bluetooth device.
#[derive(Clone, Default)]
#[allow(dead_code)]
pub struct DeviceInfo {
    pub id: String,
    pub name: String,
    pub tx_power: String,
    pub address: String,
    pub rssi: String,
    pub manufacturer_data: HashMap<u16, Vec<u8>>,
    pub services: Vec<Uuid>,
    pub detected_at: String,
    pub service_data: HashMap<Uuid, Vec<u8>>,
    pub device: Option<btleplug::platform::Peripheral>,
}

impl DeviceInfo {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: String,
        name: Option<String>,
        tx_power: Option<i16>,
        address: String,
        rssi: Option<i16>,
        manufacturer_data: HashMap<u16, Vec<u8>>,
        services: Vec<Uuid>,
        service_data: HashMap<Uuid, Vec<u8>>,
        device: btleplug::platform::Peripheral,
    ) -> Self {
        Self {
            id,
            name: name.unwrap_or_else(|| "Unknown".to_string()),
            tx_power: tx_power.map_or_else(|| "n/a".to_string(), |tx| tx.to_string()),
            address,
            rssi: rssi.map_or_else(|| "n/a".to_string(), |rssi| rssi.to_string()),
            manufacturer_data,
            services,
            detected_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            service_data,
            device: Some(device),
        }
    }

    pub fn get_id(&self) -> String {
        if cfg!(target_os = "macos") {
            self.id.clone()
        } else {
            self.address.clone()
        }
    }
}

/// A struct to hold the information of a GATT Characteristic.
#[derive(Clone)]
#[allow(dead_code)]
pub struct Characteristic {
    pub uuid: Uuid,
    pub properties: CharPropFlags,
    pub descriptors: Vec<Uuid>,
    pub service: Uuid,
    pub handle: Option<btleplug::api::Characteristic>,
}

/// A struct to hold the information of a GATT Descriptor.
pub struct ManufacturerData {
    pub company_code: String,
    pub data: String,
}

/// A struct to hold data for a CSV file.
#[derive(serde::Serialize)]
pub struct DeviceCsv {
    pub id: String,
    pub name: String,
    pub tx_power: String,
    pub address: String,
    pub rssi: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_panel_cycle() {
        let panel = FocusPanel::DeviceList;
        assert_eq!(panel.next(), FocusPanel::Characteristics);
        assert_eq!(panel.next().next(), FocusPanel::ReadWrite);
        assert_eq!(panel.next().next().next(), FocusPanel::MessageLog);
        assert_eq!(panel.next().next().next().next(), FocusPanel::DeviceList);
    }

    #[test]
    fn test_data_format_toggle() {
        let fmt = DataFormat::Text;
        assert_eq!(fmt.toggle(), DataFormat::Hex);
        assert_eq!(fmt.toggle().toggle(), DataFormat::Text);
    }

    #[test]
    fn test_data_format_label() {
        assert_eq!(DataFormat::Text.label(), "Text");
        assert_eq!(DataFormat::Hex.label(), "Hex");
    }

    #[test]
    fn test_log_direction_symbol() {
        assert_eq!(LogDirection::Sent.symbol(), "→");
        assert_eq!(LogDirection::Received.symbol(), "←");
        assert_eq!(LogDirection::Info.symbol(), "ℹ");
        assert_eq!(LogDirection::Error.symbol(), "✗");
    }

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::with_timestamp("12:00:00.000", LogDirection::Info, "test".into());
        assert_eq!(entry.timestamp, "12:00:00.000");
        assert_eq!(entry.direction, LogDirection::Info);
        assert_eq!(entry.message, "test");
    }

    #[test]
    fn test_device_info_default() {
        let device = DeviceInfo::default();
        assert_eq!(device.name, "");
        assert_eq!(device.rssi, "");
        assert!(device.device.is_none());
    }

    #[test]
    fn test_app_mode_variants() {
        assert_ne!(AppMode::Client, AppMode::Server);
    }

    #[test]
    fn test_input_mode_variants() {
        assert_ne!(InputMode::Normal, InputMode::Editing);
    }

    #[test]
    fn test_server_field_next() {
        assert_eq!(ServerField::Name.next(), ServerField::ServiceUuid);
        assert_eq!(ServerField::ServiceUuid.next(), ServerField::CharUuid);
        assert_eq!(ServerField::CharUuid.next(), ServerField::Name);
    }

    #[test]
    fn test_server_field_prev() {
        assert_eq!(ServerField::Name.prev(), ServerField::CharUuid);
        assert_eq!(ServerField::CharUuid.prev(), ServerField::ServiceUuid);
        assert_eq!(ServerField::ServiceUuid.prev(), ServerField::Name);
    }

    #[test]
    fn test_server_field_labels() {
        assert_eq!(ServerField::Name.label(), "Device Name");
        assert_eq!(ServerField::ServiceUuid.label(), "Service UUID");
        assert_eq!(ServerField::CharUuid.label(), "Char UUID");
    }
}
