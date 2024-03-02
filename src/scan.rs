use btleplug::api::{
    Central, CentralEvent, Manager as _, Peripheral, PeripheralProperties, ScanFilter,
};
use btleplug::platform::Manager;
use futures::StreamExt;
use std::error::Error;
use tokio::sync::mpsc;

use crate::structs::DeviceInfo;

pub async fn bluetooth_scan(tx: mpsc::Sender<Vec<DeviceInfo>>) -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let central = adapters.into_iter().next().ok_or("No adapters found")?;

    central.start_scan(ScanFilter::default()).await?;
    let mut events = central.events().await?;

    let mut devices_info = Vec::new();

    while let Some(event) = events.next().await {
        if let CentralEvent::DeviceDiscovered(id) = event {
            if let Ok(device) = central.peripheral(&id).await {
                let properties = device
                    .properties()
                    .await?
                    .unwrap_or(PeripheralProperties::default());
                let name = properties
                    .local_name
                    .unwrap_or_else(|| "Unknown".to_string());
                let tx_power = properties
                    .tx_power_level
                    .map_or_else(|| "N/A".to_string(), |tx| tx.to_string());
                let address = properties.address.to_string();
                let rssi = properties
                    .rssi
                    .map_or_else(|| "N/A".to_string(), |rssi| rssi.to_string());

                // Add the new device's information to the accumulated list
                devices_info.push(DeviceInfo {
                    address,
                    name,
                    tx_power,
                    rssi,
                });

                // Send a clone of the accumulated device information so far
                tx.send(devices_info.clone()).await?;
            }
        }
    }

    Ok(())
}
