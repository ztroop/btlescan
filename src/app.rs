use std::{
    error::Error,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use ratatui::widgets::TableState;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

use crate::{
    scan::{bluetooth_scan, get_characteristics},
    structs::{Characteristic, DeviceCsv, DeviceInfo},
};

pub enum DeviceData {
    DeviceInfo(DeviceInfo),
    #[allow(dead_code)]
    Characteristics(Vec<Characteristic>),
    Error(String),
}

#[allow(dead_code)]
pub struct App {
    pub rx: UnboundedReceiver<DeviceData>,
    pub tx: UnboundedSender<DeviceData>,
    pub loading_status: Arc<AtomicBool>,
    pub pause_status: Arc<AtomicBool>,
    pub table_state: TableState,
    pub devices: Vec<DeviceInfo>,
    pub inspect_view: bool,
    pub inspect_overlay_scroll: usize,
    pub selected_characteristics: Vec<Characteristic>,
    pub frame_count: usize,
    pub is_loading: bool,
    pub error_view: bool,
    pub error_message: String,
}

impl App {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            tx,
            rx,
            loading_status: Arc::new(AtomicBool::default()),
            pause_status: Arc::new(AtomicBool::default()),
            table_state: TableState::default(),
            devices: Vec::new(),
            inspect_view: false,
            inspect_overlay_scroll: 0,
            selected_characteristics: Vec::new(),
            frame_count: 0,
            is_loading: false,
            error_view: false,
            error_message: String::new(),
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
            .unwrap();

        self.pause_status.store(true, Ordering::SeqCst);

        let device = Arc::new(selected_device.clone());
        let tx_clone = self.tx.clone();

        tokio::spawn(async move { get_characteristics(tx_clone, device).await });
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
