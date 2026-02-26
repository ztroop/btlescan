use crate::app::DeviceData;
use crate::structs::{Characteristic, DeviceInfo};
use btleplug::api::{
    Central, CentralEvent, Manager as _, Peripheral, PeripheralProperties, ScanFilter, WriteType,
};
use btleplug::platform::Manager;
use futures::StreamExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::timeout;

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

                let _ = tx.send(DeviceData::DeviceInfo(Box::new(device)));
            }
        }
    }
}

/// Connects to a device, discovers services, retrieves characteristics,
/// and starts a background notification listener.
pub async fn connect_device(tx: mpsc::UnboundedSender<DeviceData>, peripheral: Arc<DeviceInfo>) {
    let duration = Duration::from_secs(10);
    match &peripheral.device {
        Some(device) => match timeout(duration, device.connect()).await {
            Ok(Ok(_)) => {
                if let Some(device) = &peripheral.device {
                    if let Err(e) = device.discover_services().await {
                        let _ = tx.send(DeviceData::Error(format!(
                            "Service discovery failed: {}",
                            e
                        )));
                        return;
                    }

                    let btleplug_chars = device.characteristics();
                    let mut result = Vec::new();
                    for c in btleplug_chars {
                        result.push(Characteristic {
                            uuid: c.uuid,
                            properties: c.properties,
                            descriptors: c.descriptors.iter().map(|d| d.uuid).collect(),
                            service: c.service_uuid,
                            handle: Some(c),
                        });
                    }
                    let _ = tx.send(DeviceData::Characteristics(result));

                    let tx_notify = tx.clone();
                    let device_clone = device.clone();
                    tokio::spawn(async move {
                        if let Ok(mut stream) = device_clone.notifications().await {
                            while let Some(notif) = stream.next().await {
                                let _ = tx_notify.send(DeviceData::Notification {
                                    uuid: notif.uuid,
                                    value: notif.value,
                                });
                            }
                        }
                    });
                }
            }
            Ok(Err(e)) => {
                let _ = tx.send(DeviceData::Error(format!("Connection error: {}", e)));
            }
            Err(_) => {
                let _ = tx.send(DeviceData::Error("Connection timed out".to_string()));
            }
        },
        None => {
            let _ = tx.send(DeviceData::Error("Device not found".to_string()));
        }
    }
}

pub async fn disconnect_device(tx: mpsc::UnboundedSender<DeviceData>, peripheral: Arc<DeviceInfo>) {
    if let Some(device) = &peripheral.device {
        match device.disconnect().await {
            Ok(_) => {
                let _ = tx.send(DeviceData::Info("Disconnected".to_string()));
            }
            Err(e) => {
                let _ = tx.send(DeviceData::Error(format!("Disconnect error: {}", e)));
            }
        }
    }
}

pub async fn read_characteristic_value(
    tx: mpsc::UnboundedSender<DeviceData>,
    device: btleplug::platform::Peripheral,
    characteristic: btleplug::api::Characteristic,
) {
    match device.read(&characteristic).await {
        Ok(value) => {
            let _ = tx.send(DeviceData::CharacteristicValue {
                uuid: characteristic.uuid,
                value,
            });
        }
        Err(e) => {
            let _ = tx.send(DeviceData::Error(format!("Read error: {}", e)));
        }
    }
}

pub async fn write_characteristic_value(
    tx: mpsc::UnboundedSender<DeviceData>,
    device: btleplug::platform::Peripheral,
    characteristic: btleplug::api::Characteristic,
    data: Vec<u8>,
) {
    match device
        .write(&characteristic, &data, WriteType::WithResponse)
        .await
    {
        Ok(_) => {
            let _ = tx.send(DeviceData::WriteComplete {
                uuid: characteristic.uuid,
            });
        }
        Err(e) => {
            let _ = tx.send(DeviceData::Error(format!("Write error: {}", e)));
        }
    }
}

pub async fn subscribe_to_notifications(
    tx: mpsc::UnboundedSender<DeviceData>,
    device: btleplug::platform::Peripheral,
    characteristic: btleplug::api::Characteristic,
) {
    match device.subscribe(&characteristic).await {
        Ok(_) => {
            let _ = tx.send(DeviceData::SubscribeComplete {
                uuid: characteristic.uuid,
            });
        }
        Err(e) => {
            let _ = tx.send(DeviceData::Error(format!("Subscribe error: {}", e)));
        }
    }
}

pub async fn unsubscribe_from_notifications(
    tx: mpsc::UnboundedSender<DeviceData>,
    device: btleplug::platform::Peripheral,
    characteristic: btleplug::api::Characteristic,
) {
    match device.unsubscribe(&characteristic).await {
        Ok(_) => {
            let _ = tx.send(DeviceData::UnsubscribeComplete {
                uuid: characteristic.uuid,
            });
        }
        Err(e) => {
            let _ = tx.send(DeviceData::Error(format!("Unsubscribe error: {}", e)));
        }
    }
}
