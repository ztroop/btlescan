use crate::app::{send_or_log, DeviceData};
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

#[allow(clippy::too_many_lines)]
pub async fn bluetooth_scan(
    tx: mpsc::UnboundedSender<DeviceData>,
    pause_signal: Arc<AtomicBool>,
    mut shutdown: tokio::sync::oneshot::Receiver<()>,
) {
    let manager = match Manager::new().await {
        Ok(m) => m,
        Err(e) => {
            send_or_log(
                &tx,
                DeviceData::Error(format!("Bluetooth manager init failed: {e}")),
            );
            return;
        }
    };
    let adapters = match manager.adapters().await {
        Ok(a) => a,
        Err(e) => {
            send_or_log(
                &tx,
                DeviceData::Error(format!("Failed to get adapters: {e}")),
            );
            return;
        }
    };
    let Some(central) = adapters.into_iter().next() else {
        send_or_log(
            &tx,
            DeviceData::Error("No Bluetooth adapters found".to_string()),
        );
        return;
    };

    if let Err(e) = central.start_scan(ScanFilter::default()).await {
        send_or_log(&tx, DeviceData::Error(format!("Scan start failed: {e}")));
        return;
    }
    let mut events = match central.events().await {
        Ok(e) => e,
        Err(e) => {
            send_or_log(
                &tx,
                DeviceData::Error(format!("Failed to get scan events: {e}")),
            );
            return;
        }
    };
    let mut scanning = true;

    loop {
        if shutdown.try_recv().is_ok() {
            if scanning {
                let _ = central.stop_scan().await;
            }
            break;
        }

        if pause_signal.load(Ordering::SeqCst) {
            if scanning {
                let _ = central.stop_scan().await;
                scanning = false;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }

        if !scanning {
            let _ = central.start_scan(ScanFilter::default()).await;
            scanning = true;
        }

        tokio::select! {
            _ = &mut shutdown => {
                if scanning {
                    let _ = central.stop_scan().await;
                }
                break;
            }
            event = events.next() => {
                match event {
                    Some(CentralEvent::DeviceDiscovered(id) | CentralEvent::DeviceUpdated(id)) => {
                        if let Ok(device) = central.peripheral(&id).await {
                            let properties = device
                                .properties()
                                .await
                                .ok()
                                .flatten()
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

                            send_or_log(&tx, DeviceData::DeviceInfo(Box::new(device)));
                        }
                    }
                    Some(_) => {}
                    None => break,
                }
            }
            () = tokio::time::sleep(Duration::from_millis(200)) => {}
        }
    }
    // Manager, central, and events stream are dropped here,
    // fully releasing the CBCentralManager.
}

/// Connects to a device, discovers services, retrieves characteristics,
/// and starts a background notification listener.
pub async fn connect_device(tx: mpsc::UnboundedSender<DeviceData>, peripheral: Arc<DeviceInfo>) {
    let duration = Duration::from_secs(10);
    match &peripheral.device {
        Some(device) => match timeout(duration, device.connect()).await {
            Ok(Ok(())) => {
                if let Some(device) = &peripheral.device {
                    if let Err(e) = device.discover_services().await {
                        let _ = device.disconnect().await;
                        send_or_log(
                            &tx,
                            DeviceData::Error(format!("Service discovery failed: {e}")),
                        );
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
                    send_or_log(
                        &tx,
                        DeviceData::Characteristics {
                            device_id: peripheral.get_id(),
                            characteristics: result,
                        },
                    );

                    // Notification task runs until the stream ends (device disconnect).
                    // Not explicitly cancelled on disconnect; the stream ends when the
                    // peripheral disconnects, so the task will finish shortly after.
                    let tx_notify = tx.clone();
                    let device_clone = device.clone();
                    tokio::spawn(async move {
                        match device_clone.notifications().await {
                            Ok(mut stream) => {
                                while let Some(notif) = stream.next().await {
                                    send_or_log(
                                        &tx_notify,
                                        DeviceData::Notification {
                                            uuid: notif.uuid,
                                            value: notif.value,
                                        },
                                    );
                                }
                            }
                            Err(e) => {
                                send_or_log(
                                    &tx_notify,
                                    DeviceData::Error(format!("Notification stream failed: {e}")),
                                );
                            }
                        }
                    });
                }
            }
            Ok(Err(e)) => {
                send_or_log(&tx, DeviceData::Error(format!("Connection error: {e}")));
            }
            Err(_) => {
                send_or_log(&tx, DeviceData::Error("Connection timed out".to_string()));
            }
        },
        None => {
            send_or_log(&tx, DeviceData::Error("Device not found".to_string()));
        }
    }
}

pub async fn disconnect_device(tx: mpsc::UnboundedSender<DeviceData>, peripheral: Arc<DeviceInfo>) {
    if let Some(device) = &peripheral.device {
        match device.disconnect().await {
            Ok(()) => {
                send_or_log(&tx, DeviceData::Info("Disconnected".to_string()));
            }
            Err(e) => {
                send_or_log(&tx, DeviceData::Error(format!("Disconnect error: {e}")));
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
            send_or_log(
                &tx,
                DeviceData::CharacteristicValue {
                    uuid: characteristic.uuid,
                    value,
                },
            );
        }
        Err(e) => {
            send_or_log(&tx, DeviceData::Error(format!("Read error: {e}")));
        }
    }
}

pub async fn write_characteristic_value(
    tx: mpsc::UnboundedSender<DeviceData>,
    device: btleplug::platform::Peripheral,
    characteristic: btleplug::api::Characteristic,
    data: Vec<u8>,
) {
    let result = device
        .write(&characteristic, &data, WriteType::WithResponse)
        .await;

    let (result, error_msg) = match result {
        Ok(()) => (Ok(()), None),
        Err(e1) => {
            let first_err = e1.to_string();
            match device
                .write(&characteristic, &data, WriteType::WithoutResponse)
                .await
            {
                Ok(()) => (Ok(()), None),
                Err(e2) => {
                    let second_err = e2.to_string();
                    (
                        Err(e2),
                        Some(format!(
                            "WithResponse: {first_err}; WithoutResponse: {second_err}"
                        )),
                    )
                }
            }
        }
    };

    if let Ok(()) = result {
        send_or_log(
            &tx,
            DeviceData::WriteComplete {
                uuid: characteristic.uuid,
            },
        );
    } else {
        let msg = error_msg.unwrap_or_else(|| "Write failed".to_string());
        send_or_log(&tx, DeviceData::Error(format!("Write error: {msg}")));
    }
}

pub async fn subscribe_to_notifications(
    tx: mpsc::UnboundedSender<DeviceData>,
    device: btleplug::platform::Peripheral,
    characteristic: btleplug::api::Characteristic,
) {
    match device.subscribe(&characteristic).await {
        Ok(()) => {
            send_or_log(
                &tx,
                DeviceData::SubscribeComplete {
                    uuid: characteristic.uuid,
                },
            );
        }
        Err(e) => {
            send_or_log(&tx, DeviceData::Error(format!("Subscribe error: {e}")));
        }
    }
}

pub async fn unsubscribe_from_notifications(
    tx: mpsc::UnboundedSender<DeviceData>,
    device: btleplug::platform::Peripheral,
    characteristic: btleplug::api::Characteristic,
) {
    match device.unsubscribe(&characteristic).await {
        Ok(()) => {
            send_or_log(
                &tx,
                DeviceData::UnsubscribeComplete {
                    uuid: characteristic.uuid,
                },
            );
        }
        Err(e) => {
            send_or_log(&tx, DeviceData::Error(format!("Unsubscribe error: {e}")));
        }
    }
}
