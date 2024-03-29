use crate::app::DeviceData;
use crate::structs::{Characteristic, DeviceInfo};
use btleplug::api::{
    Central, CentralEvent, Manager as _, Peripheral, PeripheralProperties, ScanFilter,
};
use btleplug::platform::Manager;
use futures::StreamExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

/// Scans for Bluetooth devices and sends the information to the provided `mpsc::Sender`.
/// The scan can be paused by setting the `pause_signal` to `true`.
pub async fn bluetooth_scan(tx: mpsc::UnboundedSender<DeviceData>, pause_signal: Arc<AtomicBool>) {
    let manager = Manager::new().await.unwrap();
    let adapters = manager.adapters().await.unwrap();
    let central = adapters.into_iter().next().expect("No adapters found");

    central
        .start_scan(ScanFilter::default())
        .await
        .expect("Scanning failure");
    let mut events = central.events().await.unwrap();

    while let Some(event) = events.next().await {
        // Check the pause signal before processing the event
        while pause_signal.load(Ordering::SeqCst) {
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        if let CentralEvent::DeviceDiscovered(id) = event {
            if let Ok(device) = central.peripheral(&id).await {
                let properties = device
                    .properties()
                    .await
                    .unwrap()
                    .unwrap_or(PeripheralProperties::default());

                // Add the new device's information to the accumulated list
                let device = DeviceInfo::new(
                    device.id().to_string(),
                    properties.local_name,
                    properties.tx_power_level,
                    properties.address.to_string(),
                    properties.rssi,
                    properties.manufacturer_data,
                    properties.services,
                    properties.service_data,
                    device.clone(),
                );

                // Send a clone of the accumulated device information so far
                let _ = tx.send(DeviceData::DeviceInfo(device));
            }
        }
    }
}

/// Gets the characteristics of a Bluetooth device and returns them as a `Vec<Characteristic>`.
/// The device is identified by its address or UUID.
pub async fn get_characteristics(
    tx: mpsc::UnboundedSender<DeviceData>,
    peripheral: Arc<DeviceInfo>,
) {
    let duration = Duration::from_secs(10);
    match &peripheral.device {
        Some(device) => match timeout(duration, device.connect()).await {
            Ok(Ok(_)) => {
                if let Some(device) = &peripheral.device {
                    let characteristics = device.characteristics();
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
                            service: characteristic.service_uuid,
                        });
                    }
                    let _ = tx.send(DeviceData::Characteristics(result));
                }
            }
            Ok(Err(e)) => {
                tx.send(DeviceData::Error(format!("Connection error: {}", e)))
                    .unwrap();
            }
            Err(_) => {
                tx.send(DeviceData::Error("Connection timed out".to_string()))
                    .unwrap();
            }
        },
        None => {
            tx.send(DeviceData::Error("Device not found".to_string()))
                .unwrap();
        }
    }
}
