use crate::aqua::connection::response::Response;
use dashmap::DashMap;
use mqtt_coder::mqtt::{
    self, ClientId, ControlPacket, MqttError, MqttPacket, PacketId, Publish, QoS,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tracing::trace;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Outbound {
    tx: mpsc::Sender<Response>,
}

impl Outbound {
    pub fn new(tx: mpsc::Sender<Response>) -> Self {
        Self { tx }
    }

    /// ControlPacket を送信
    pub fn send(&self, pkt: ControlPacket) -> Result<(), TrySendError<Response>> {
        self.tx.try_send(Response::new(pkt))
    }
}

#[derive(Clone)]
pub struct SessionManager {
    /* client id : Outbound */
    by_client_id: Arc<DashMap<Uuid, Outbound>>,
    by_mqtt_id: Arc<DashMap<String, Uuid>>,
    by_client_mqtt: Arc<DashMap<Uuid, String>>,
    qos_tmp: Arc<DashMap<u16, (Publish, QoS)>>,
    mqtt_version: Arc<DashMap<String, mqtt::ProtocolVersion>>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            by_client_id: Arc::new(DashMap::new()),
            by_mqtt_id: Arc::new(DashMap::new()),
            by_client_mqtt: Arc::new(DashMap::new()),
            qos_tmp: Arc::new(DashMap::new()),
            mqtt_version: Arc::new(DashMap::new()),
        }
    }

    // for re-send, for QoS2
    pub fn add_staging_packet(&self, pkt: Publish, qos: QoS) {
        if let Some(ref id) = pkt.packet_id {
            self.qos_tmp.insert(id.value().clone(), (pkt, qos));
        }
    }
    pub fn fetch_packet(&self, pid: PacketId) -> Result<Publish, MqttError> {
        if let Some(entry) = self.qos_tmp.get(&pid.value()) {
            let (publish, _) = entry.value();
            return Ok(publish.clone());
        } else {
            return Err(MqttError::Unexpected);
        }
    }

    pub fn commit_packet(&self, pid: PacketId) -> Result<(), MqttError> {
        if let Some(_) = self.qos_tmp.remove(&pid.value()) {
            return Ok(());
        }
        return Err(MqttError::Unexpected);
    }

    pub fn register_client_id(&self, client_id: Uuid, outbound: Outbound) {
        trace!("register_client_id {:?}", client_id);
        self.by_client_id.insert(client_id, outbound);
    }
    pub fn unregister_client_id(&self, client_id: Uuid) {
        trace!("unregister_client_id {:?}", client_id);
        self.by_client_id.remove(&client_id);
    }
    pub fn send_by_client_id(
        &self,
        client_id: &Uuid,
        pkt: ControlPacket,
    ) -> Result<(), TrySendError<Response>> {
        trace!("sent_by_client_id {:?}", client_id);
        if let Some(outbound) = self.by_client_id.get(client_id) {
            outbound.send(pkt)
        } else {
            Err(TrySendError::Closed(Response::new(pkt)))
        }
    }

    pub fn send_by_mqtt_id(
        &self,
        mqtt_id: &String,
        pkt: ControlPacket,
    ) -> Result<(), TrySendError<Response>> {
        trace!("sent_by_mqtt_id {:?}", mqtt_id);
        trace!("client_id map  {:?}", self.by_client_id);
        trace!("mqtt_id map  {:?}", self.by_client_mqtt);
        if let Some(value_ref) = self.by_mqtt_id.get(mqtt_id) {
            let client_id = *value_ref;
            trace!("send, client_id {:?}", client_id);
            self.send_by_client_id(&client_id, pkt)
        } else {
            trace!("cannot send anything");
            Ok(())
        }
    }

    pub fn register_mqtt_id(&self, mqtt_id: String, client_id: Uuid) {
        trace!("register_mqtt_id {:?} {:?}", mqtt_id, client_id);
        self.by_mqtt_id.insert(mqtt_id.clone(), client_id);
        self.by_client_mqtt.insert(client_id, mqtt_id);
    }
    pub fn unregister_mqtt_id(&self, mqtt_id: String) {
        if let Some((_, client_id)) = self.by_mqtt_id.remove(&mqtt_id) {
            self.by_client_mqtt.remove(&client_id);
        }
    }
    pub fn get_mqtt_id(&self, client_id: &Uuid) -> Option<String> {
        self.by_client_mqtt
            .get(client_id)
            .map(|r| r.value().clone())
    }
    pub fn get_protocol_version(&self, mqtt_id: &str) -> Option<mqtt::ProtocolVersion> {
        self.mqtt_version
            .get(mqtt_id)
            .map(|entry| entry.value().clone())
    }
    pub fn set_protocol_version(&self, mqtt_id: &str, version: mqtt::ProtocolVersion) {
        self.mqtt_version.insert(mqtt_id.to_string(), version);
    }
}
