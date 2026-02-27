use ble_peripheral_rust::{
    gatt::{
        characteristic::Characteristic as BleCharacteristic,
        peripheral_event::{
            PeripheralEvent, ReadRequestResponse, RequestResponse, WriteRequestResponse,
        },
        properties::{AttributePermission, CharacteristicProperty},
        service::Service,
    },
    Peripheral, PeripheralImpl,
};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::app::DeviceData;
use crate::structs::LogDirection;

pub struct ServerHandle {
    peripheral: Peripheral,
    service_uuid: Uuid,
    char_uuid: Uuid,
}

#[allow(dead_code)]
impl ServerHandle {
    pub async fn stop(&mut self) {
        let _ = self.peripheral.stop_advertising().await;
    }

    pub async fn update_value(&mut self, data: Vec<u8>) -> Result<(), String> {
        self.peripheral
            .update_characteristic(self.char_uuid, data)
            .await
            .map_err(|e| format!("Update error: {}", e))
    }

    pub fn service_uuid(&self) -> Uuid {
        self.service_uuid
    }

    pub fn char_uuid(&self) -> Uuid {
        self.char_uuid
    }
}

/// Starts the GATT server, adds a default service, begins advertising,
/// and spawns a task to forward peripheral events to the app channel.
pub async fn start_server(
    app_tx: mpsc::UnboundedSender<DeviceData>,
    server_name: String,
    service_uuid: Uuid,
    char_uuid: Uuid,
) -> Result<ServerHandle, String> {
    let (event_tx, mut event_rx) = mpsc::channel::<PeripheralEvent>(256);

    let mut peripheral = Peripheral::new(event_tx)
        .await
        .map_err(|e| format!("Failed to create peripheral: {}", e))?;

    let mut retries = 0;
    while !peripheral
        .is_powered()
        .await
        .map_err(|e| format!("Power check failed: {}", e))?
    {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        retries += 1;
        if retries > 50 {
            return Err("Bluetooth adapter not powered on".into());
        }
    }

    let service = Service {
        uuid: service_uuid,
        primary: true,
        characteristics: vec![BleCharacteristic {
            uuid: char_uuid,
            properties: vec![
                CharacteristicProperty::Read,
                CharacteristicProperty::Write,
                CharacteristicProperty::Notify,
            ],
            permissions: vec![
                AttributePermission::Readable,
                AttributePermission::Writeable,
            ],
            value: None,
            descriptors: vec![],
        }],
    };

    peripheral
        .add_service(&service)
        .await
        .map_err(|e| format!("Failed to add service: {}", e))?;

    peripheral
        .start_advertising(&server_name, &[service_uuid])
        .await
        .map_err(|e| format!("Failed to start advertising: {}", e))?;

    let tx = app_tx.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            handle_peripheral_event(event, &tx);
        }
    });

    Ok(ServerHandle {
        peripheral,
        service_uuid,
        char_uuid,
    })
}

fn handle_peripheral_event(event: PeripheralEvent, tx: &mpsc::UnboundedSender<DeviceData>) {
    match event {
        PeripheralEvent::StateUpdate { is_powered } => {
            let msg = if is_powered {
                "Bluetooth adapter powered on".into()
            } else {
                "Bluetooth adapter powered off".into()
            };
            let _ = tx.send(DeviceData::ServerLog {
                direction: LogDirection::Info,
                message: msg,
            });
        }
        PeripheralEvent::CharacteristicSubscriptionUpdate {
            request,
            subscribed,
        } => {
            let action = if subscribed {
                "subscribed to"
            } else {
                "unsubscribed from"
            };
            let _ = tx.send(DeviceData::ServerLog {
                direction: LogDirection::Info,
                message: format!("Client {} {}", action, request.characteristic),
            });
        }
        PeripheralEvent::ReadRequest {
            request,
            offset,
            responder,
        } => {
            let _ = tx.send(DeviceData::ServerLog {
                direction: LogDirection::Received,
                message: format!(
                    "Read request on {} (offset: {})",
                    request.characteristic, offset
                ),
            });
            let _ = responder.send(ReadRequestResponse {
                value: vec![],
                response: RequestResponse::Success,
            });
        }
        PeripheralEvent::WriteRequest {
            request,
            offset,
            value,
            responder,
        } => {
            let hex: String = value
                .iter()
                .map(|b| format!("{:02X}", b))
                .collect::<Vec<_>>()
                .join(" ");
            let _ = tx.send(DeviceData::ServerLog {
                direction: LogDirection::Received,
                message: format!(
                    "Write request on {} (offset: {}): {}",
                    request.characteristic, offset, hex
                ),
            });
            let _ = responder.send(WriteRequestResponse {
                response: RequestResponse::Success,
            });
        }
    }
}
