use std::collections::HashMap;

use btleplug::api::CharPropFlags;
use uuid::Uuid;

/// A struct to hold the information of a Bluetooth device.
#[derive(Clone, Default)]
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
}

impl DeviceInfo {
    /// Creates a new `DeviceInfo` with the provided information.
    pub fn new(
        id: String,
        name: Option<String>,
        tx_power: Option<i16>,
        address: String,
        rssi: Option<i16>,
        manufacturer_data: HashMap<u16, Vec<u8>>,
        services: Vec<Uuid>,
        service_data: HashMap<Uuid, Vec<u8>>,
    ) -> Self {
        DeviceInfo {
            id,
            name: name.unwrap_or_else(|| "Unknown".to_string()),
            tx_power: tx_power.map_or_else(|| "n/a".to_string(), |tx| tx.to_string()),
            address,
            rssi: rssi.map_or_else(|| "n/a".to_string(), |rssi| rssi.to_string()),
            manufacturer_data,
            services,
            detected_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            service_data,
        }
    }
}

pub struct Characteristic {
    pub uuid: Uuid,
    pub properties: CharPropFlags,
    pub descriptors: Vec<Uuid>,
}

pub struct ManufacturerData {
    pub company_code: String,
    pub data: String,
}
