use std::sync::Arc;

use parking_lot::Mutex;

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

use crate::app::{send_or_log, DeviceData};
use crate::structs::LogDirection;

pub struct ServerHandle {
    peripheral: Peripheral,
    service_uuid: Uuid,
    char_uuid: Uuid,
    shared_value: Arc<Mutex<Vec<u8>>>,
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
            .map_err(|e| format!("Update error: {e}"))
    }

    pub fn set_value(&self, data: Vec<u8>) {
        *self.shared_value.lock() = data;
    }

    pub fn get_value(&self) -> Vec<u8> {
        self.shared_value.lock().clone()
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
    shared_value: Arc<Mutex<Vec<u8>>>,
) -> Result<ServerHandle, String> {
    let (event_tx, mut event_rx) = mpsc::channel::<PeripheralEvent>(256);

    let mut peripheral = Peripheral::new(event_tx)
        .await
        .map_err(|e| format!("Failed to create peripheral: {e}"))?;

    let mut retries = 0;
    while !peripheral
        .is_powered()
        .await
        .map_err(|e| format!("Power check failed: {e}"))?
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
        .map_err(|e| format!("Failed to add service: {e}"))?;

    peripheral
        .start_advertising(&server_name, &[service_uuid])
        .await
        .map_err(|e| format!("Failed to start advertising: {e}"))?;

    let tx = app_tx.clone();
    let value_ref = Arc::clone(&shared_value);
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            handle_peripheral_event(event, &tx, &value_ref);
        }
    });

    Ok(ServerHandle {
        peripheral,
        service_uuid,
        char_uuid,
        shared_value,
    })
}

#[allow(clippy::too_many_lines)]
fn handle_peripheral_event(
    event: PeripheralEvent,
    tx: &mpsc::UnboundedSender<DeviceData>,
    shared_value: &Arc<Mutex<Vec<u8>>>,
) {
    match event {
        PeripheralEvent::StateUpdate { is_powered } => {
            let msg = if is_powered {
                "Bluetooth adapter powered on".into()
            } else {
                "Bluetooth adapter powered off".into()
            };
            send_or_log(
                tx,
                DeviceData::ServerLog {
                    direction: LogDirection::Info,
                    message: msg,
                },
            );
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
            send_or_log(
                tx,
                DeviceData::ServerLog {
                    direction: LogDirection::Info,
                    message: format!("Client {action} {}", request.characteristic),
                },
            );
        }
        PeripheralEvent::ReadRequest {
            request,
            offset,
            responder,
        } => {
            let full = shared_value.lock().clone();
            let offset_usize = offset.try_into().unwrap_or(usize::MAX).min(full.len());
            let value = if offset_usize >= full.len() {
                vec![]
            } else {
                full[offset_usize..].to_vec()
            };
            let hex: String = value
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            send_or_log(
                tx,
                DeviceData::ServerLog {
                    direction: LogDirection::Received,
                    message: format!(
                        "Read request on {} (offset: {offset}), responding: {}",
                        request.characteristic,
                        if hex.is_empty() { "(empty)" } else { &hex }
                    ),
                },
            );
            let _ = responder.send(ReadRequestResponse {
                value,
                response: RequestResponse::Success,
            });
        }
        PeripheralEvent::WriteRequest {
            request,
            offset,
            value,
            responder,
        } => {
            const MAX_CHARACTERISTIC_SIZE: usize = 512;
            let offset_usize: usize = offset.try_into().unwrap_or(usize::MAX);
            if offset_usize > MAX_CHARACTERISTIC_SIZE
                || offset_usize.saturating_add(value.len()) > MAX_CHARACTERISTIC_SIZE
            {
                let _ = responder.send(WriteRequestResponse {
                    response: RequestResponse::InvalidOffset,
                });
                return;
            }
            let hex: String = value
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" ");
            send_or_log(
                tx,
                DeviceData::ServerLog {
                    direction: LogDirection::Received,
                    message: format!(
                        "Write request on {} (offset: {offset}): {hex}",
                        request.characteristic
                    ),
                },
            );
            {
                let mut guard = shared_value.lock();
                if offset_usize == 0 {
                    *guard = value;
                } else {
                    let end = offset_usize + value.len();
                    if end > guard.len() {
                        guard.resize(end, 0);
                    }
                    guard[offset_usize..end].copy_from_slice(&value);
                }
            }
            let _ = responder.send(WriteRequestResponse {
                response: RequestResponse::Success,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ble_peripheral_rust::gatt::peripheral_event::PeripheralRequest;
    use tokio::sync::oneshot;

    fn make_request() -> PeripheralRequest {
        PeripheralRequest {
            client: "test-client".to_string(),
            service: Uuid::parse_str("0000180d-0000-1000-8000-00805f9b34fb").unwrap(),
            characteristic: Uuid::parse_str("00002a37-0000-1000-8000-00805f9b34fb").unwrap(),
        }
    }

    fn make_shared_value(data: Vec<u8>) -> Arc<Mutex<Vec<u8>>> {
        Arc::new(Mutex::new(data))
    }

    fn recv_server_log(rx: &mut mpsc::UnboundedReceiver<DeviceData>) -> (LogDirection, String) {
        match rx.try_recv().unwrap() {
            DeviceData::ServerLog { direction, message } => (direction, message),
            _ => panic!("Expected ServerLog variant"),
        }
    }

    #[test]
    fn test_state_update_powered_on() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![]);

        handle_peripheral_event(
            PeripheralEvent::StateUpdate { is_powered: true },
            &tx,
            &shared,
        );

        let (direction, message) = recv_server_log(&mut rx);
        assert_eq!(direction, LogDirection::Info);
        assert_eq!(message, "Bluetooth adapter powered on");
    }

    #[test]
    fn test_state_update_powered_off() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![]);

        handle_peripheral_event(
            PeripheralEvent::StateUpdate { is_powered: false },
            &tx,
            &shared,
        );

        let (direction, message) = recv_server_log(&mut rx);
        assert_eq!(direction, LogDirection::Info);
        assert_eq!(message, "Bluetooth adapter powered off");
    }

    #[test]
    fn test_subscription_subscribed() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![]);

        handle_peripheral_event(
            PeripheralEvent::CharacteristicSubscriptionUpdate {
                request: make_request(),
                subscribed: true,
            },
            &tx,
            &shared,
        );

        let (direction, message) = recv_server_log(&mut rx);
        assert_eq!(direction, LogDirection::Info);
        assert!(message.contains("subscribed to"));
        assert!(message.contains("00002a37"));
    }

    #[test]
    fn test_subscription_unsubscribed() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![]);

        handle_peripheral_event(
            PeripheralEvent::CharacteristicSubscriptionUpdate {
                request: make_request(),
                subscribed: false,
            },
            &tx,
            &shared,
        );

        let (direction, message) = recv_server_log(&mut rx);
        assert_eq!(direction, LogDirection::Info);
        assert!(message.contains("unsubscribed from"));
        assert!(message.contains("00002a37"));
    }

    #[test]
    fn test_read_request_empty_value() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![]);
        let (resp_tx, mut resp_rx) = oneshot::channel();

        handle_peripheral_event(
            PeripheralEvent::ReadRequest {
                request: make_request(),
                offset: 0,
                responder: resp_tx,
            },
            &tx,
            &shared,
        );

        let (direction, message) = recv_server_log(&mut rx);
        assert_eq!(direction, LogDirection::Received);
        assert!(message.contains("(empty)"));

        let response = resp_rx.try_recv().unwrap();
        assert!(response.value.is_empty());
        assert_eq!(response.response, RequestResponse::Success);
    }

    #[test]
    fn test_read_request_returns_shared_value() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![0xDE, 0xAD]);
        let (resp_tx, mut resp_rx) = oneshot::channel();

        handle_peripheral_event(
            PeripheralEvent::ReadRequest {
                request: make_request(),
                offset: 0,
                responder: resp_tx,
            },
            &tx,
            &shared,
        );

        let (direction, message) = recv_server_log(&mut rx);
        assert_eq!(direction, LogDirection::Received);
        assert!(message.contains("DE AD"));

        let response = resp_rx.try_recv().unwrap();
        assert_eq!(response.value, vec![0xDE, 0xAD]);
        assert_eq!(response.response, RequestResponse::Success);
    }

    #[test]
    fn test_read_request_includes_offset() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![0xAB]);
        let (resp_tx, _) = oneshot::channel();

        handle_peripheral_event(
            PeripheralEvent::ReadRequest {
                request: make_request(),
                offset: 5,
                responder: resp_tx,
            },
            &tx,
            &shared,
        );

        let (_, message) = recv_server_log(&mut rx);
        assert!(message.contains("offset: 5"));
    }

    #[test]
    fn test_write_request() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![]);
        let (resp_tx, mut resp_rx) = oneshot::channel();

        handle_peripheral_event(
            PeripheralEvent::WriteRequest {
                request: make_request(),
                offset: 0,
                value: vec![0xFF, 0x00],
                responder: resp_tx,
            },
            &tx,
            &shared,
        );

        let (direction, message) = recv_server_log(&mut rx);
        assert_eq!(direction, LogDirection::Received);
        assert!(message.contains("FF 00"));
        assert!(message.contains("00002a37"));

        let response = resp_rx.try_recv().unwrap();
        assert_eq!(response.response, RequestResponse::Success);
    }

    #[test]
    fn test_write_request_updates_shared_value() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![]);
        let (resp_tx, mut resp_rx) = oneshot::channel();

        handle_peripheral_event(
            PeripheralEvent::WriteRequest {
                request: make_request(),
                offset: 0,
                value: vec![0xDE, 0xAD, 0xBE, 0xEF],
                responder: resp_tx,
            },
            &tx,
            &shared,
        );

        let _ = recv_server_log(&mut rx);
        let response = resp_rx.try_recv().unwrap();
        assert_eq!(response.response, RequestResponse::Success);
        assert_eq!(*shared.lock(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn test_write_request_includes_offset() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![]);
        let (resp_tx, _) = oneshot::channel();

        handle_peripheral_event(
            PeripheralEvent::WriteRequest {
                request: make_request(),
                offset: 10,
                value: vec![0x01],
                responder: resp_tx,
            },
            &tx,
            &shared,
        );

        let (_, message) = recv_server_log(&mut rx);
        assert!(message.contains("offset: 10"));
    }

    #[test]
    fn test_shared_value_initially_empty() {
        let shared = make_shared_value(vec![]);
        assert!(shared.lock().is_empty());
    }

    #[test]
    #[allow(clippy::similar_names)]
    fn test_shared_value_update_reflected_in_reads() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![]);

        let (resp_tx1, mut resp_rx1) = oneshot::channel();
        handle_peripheral_event(
            PeripheralEvent::ReadRequest {
                request: make_request(),
                offset: 0,
                responder: resp_tx1,
            },
            &tx,
            &shared,
        );
        let response1 = resp_rx1.try_recv().unwrap();
        assert!(response1.value.is_empty());
        let _ = recv_server_log(&mut rx);

        *shared.lock() = vec![0xCA, 0xFE];

        let (resp_tx2, mut resp_rx2) = oneshot::channel();
        handle_peripheral_event(
            PeripheralEvent::ReadRequest {
                request: make_request(),
                offset: 0,
                responder: resp_tx2,
            },
            &tx,
            &shared,
        );
        let response2 = resp_rx2.try_recv().unwrap();
        assert_eq!(response2.value, vec![0xCA, 0xFE]);
    }

    #[test]
    fn test_write_request_empty_value() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![]);
        let (resp_tx, mut resp_rx) = oneshot::channel();

        handle_peripheral_event(
            PeripheralEvent::WriteRequest {
                request: make_request(),
                offset: 0,
                value: vec![],
                responder: resp_tx,
            },
            &tx,
            &shared,
        );

        let (direction, _) = recv_server_log(&mut rx);
        assert_eq!(direction, LogDirection::Received);

        let response = resp_rx.try_recv().unwrap();
        assert_eq!(response.response, RequestResponse::Success);
    }

    #[test]
    fn test_write_request_rejects_offset_exceeding_max() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![]);
        let (resp_tx, mut resp_rx) = oneshot::channel();

        handle_peripheral_event(
            PeripheralEvent::WriteRequest {
                request: make_request(),
                offset: 513,
                value: vec![0x01],
                responder: resp_tx,
            },
            &tx,
            &shared,
        );

        let response = resp_rx.try_recv().unwrap();
        assert_eq!(response.response, RequestResponse::InvalidOffset);
        assert!(shared.lock().is_empty());
    }

    #[test]
    fn test_write_request_rejects_offset_plus_value_exceeding_max() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let shared = make_shared_value(vec![0u8; 100]);
        let (resp_tx, mut resp_rx) = oneshot::channel();

        handle_peripheral_event(
            PeripheralEvent::WriteRequest {
                request: make_request(),
                offset: 450,
                value: vec![0u8; 100],
                responder: resp_tx,
            },
            &tx,
            &shared,
        );

        let response = resp_rx.try_recv().unwrap();
        assert_eq!(response.response, RequestResponse::InvalidOffset);
        assert_eq!(shared.lock().len(), 100);
    }
}
