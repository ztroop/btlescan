#[cfg(target_os = "linux")]
use bluez_async::MacAddress;
use btleplug::api::{
    Central, CentralEvent, Manager as _, Peripheral, PeripheralProperties, ScanFilter,
};
use btleplug::platform::{Manager, PeripheralId};
use futures::StreamExt;
use std::error::Error;
#[cfg(target_os = "linux")]
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
#[cfg(target_os = "macos")]
use uuid::Uuid;

use crate::structs::{Characteristic, DeviceInfo};

/// Scans for Bluetooth devices and sends the information to the provided `mpsc::Sender`.
/// The scan can be paused by setting the `pause_signal` to `true`.
pub async fn bluetooth_scan(
    tx: mpsc::Sender<Vec<DeviceInfo>>,
    pause_signal: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error>> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let central = adapters.into_iter().next().ok_or("No adapters found")?;

    central.start_scan(ScanFilter::default()).await?;
    let mut events = central.events().await?;

    let mut devices_info = Vec::new();

    while let Some(event) = events.next().await {
        // Check the pause signal before processing the event
        while pause_signal.load(Ordering::SeqCst) {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        if let CentralEvent::DeviceDiscovered(id) = event {
            if let Ok(device) = central.peripheral(&id).await {
                let properties = device
                    .properties()
                    .await?
                    .unwrap_or(PeripheralProperties::default());

                // Add the new device's information to the accumulated list
                devices_info.push(DeviceInfo::new(
                    device.id().to_string(),
                    properties.local_name,
                    properties.tx_power_level,
                    properties.address.to_string(),
                    properties.rssi,
                    properties.manufacturer_data,
                    properties.services,
                    properties.service_data,
                ));

                // Send a clone of the accumulated device information so far
                tx.send(devices_info.clone()).await?;
            }
        }
    }

    Ok(())
}

/// Gets the characteristics of a Bluetooth device and returns them as a `Vec<Characteristic>`.
/// The device is identified by its address or UUID.
pub async fn get_characteristics(id: &str) -> Result<Vec<Characteristic>, Box<dyn Error>> {
    let manager = Manager::new().await?;
    let adapters = manager.adapters().await?;
    let central = adapters.into_iter().next().ok_or("No adapters found")?;

    let peripheral_id: Option<PeripheralId>;
    #[cfg(target_os = "macos")]
    {
        let uuid_id = Uuid::parse_str(id)?;
        peripheral_id = Some(PeripheralId::from(uuid_id));
    }
    #[cfg(target_os = "linux")]
    {
        let mac_addr = MacAddress::from_str(id)?;
        peripheral_id = Some(PeripheralId::from(mac_addr));
    }

    // Short circuit if no peripheral is not found
    if peripheral_id.is_none() {
        return Err("Peripheral not found".into());
    }

    let peripheral = central.peripheral(&peripheral_id.unwrap()).await?;
    peripheral.connect().await?;

    let characteristics = peripheral.characteristics();
    let mut result = Vec::new();
    for characteristic in characteristics {
        result.push(Characteristic {
            uuid: characteristic.uuid,
            properties: characteristic.properties,
            descriptors: characteristic
                .descriptors
                .into_iter()
                .map(|d| d.uuid)
                .collect(),
        });
    }
    Ok(result)
}
