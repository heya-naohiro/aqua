use crate::aqua::connection::{response::Response, WriteRequest};
use dashmap::DashMap;
use futures_util::io::Write;
use log::error;
use mqtt_coder::mqtt::{
    self, ClientId, ControlPacket, MqttError, MqttPacket, PacketId, Publish, QoS,
};
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tracing::{debug, trace};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct Outbound {
    tx: mpsc::Sender<WriteRequest>,
}

impl Outbound {
    pub fn new(tx: mpsc::Sender<WriteRequest>) -> Self {
        Self { tx }
    }

    /// ControlPacket を送信
    pub fn send(&self, req: WriteRequest) -> Result<(), TrySendError<WriteRequest>> {
        self.tx.try_send(req)
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
    // [TODO] now unlimited..., need limiter
    queue_by_mqtt_id: Arc<DashMap<String, VecDeque<mqtt::ControlPacket>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            by_client_id: Arc::new(DashMap::new()),
            by_mqtt_id: Arc::new(DashMap::new()),
            by_client_mqtt: Arc::new(DashMap::new()),
            qos_tmp: Arc::new(DashMap::new()),
            mqtt_version: Arc::new(DashMap::new()),
            queue_by_mqtt_id: Arc::new(DashMap::new()),
        }
    }

    pub fn discard_queue(&self, mqtt_id: String) -> Result<(), MqttError> {
        debug!("discard queue {:?}", &mqtt_id);
        self.queue_by_mqtt_id.remove(&mqtt_id);
        Ok(())
    }
    pub fn add_to_queue(&self, mqtt_id: String, packet: mqtt::ControlPacket) {
        if let Some(mut queue) = self.queue_by_mqtt_id.get_mut(&mqtt_id) {
            debug!("exist queue, so pushback {:?}", mqtt_id);
            queue.push_back(packet);
        } else {
            debug!("new queue {:?}", mqtt_id);
            let mut new_queue = VecDeque::new();
            new_queue.push_back(packet); // ここで追加
            self.queue_by_mqtt_id.insert(mqtt_id, new_queue);
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

    // queued if clean session is false
    pub fn send_by_client_id(
        &self,
        client_id: &Uuid,
        req: WriteRequest,
    ) -> Result<(), TrySendError<WriteRequest>> {
        debug!("sent_by_client_id {:?}", client_id);
        if let Some(outbound) = self.by_client_id.get(client_id) {
            outbound.send(req)
        } else {
            Err(TrySendError::Closed(req))
        }
    }

    pub fn flush_queue(&self, mqtt_id: &String) -> VecDeque<mqtt::ControlPacket> {
        debug!("flush_queue {:?}", mqtt_id);
        self.queue_by_mqtt_id
            .remove(mqtt_id)
            .map(|(_, queue)| queue)
            .unwrap_or_else(VecDeque::new)
    }

    pub fn send_by_mqtt_id(
        &self,
        mqtt_id: &String,
        pkt: ControlPacket,
    ) -> Result<(), TrySendError<ControlPacket>> {
        debug!("sent_by_mqtt_id {:?}", mqtt_id);
        if let Some(value_ref) = self.by_mqtt_id.get(mqtt_id) {
            let client_id = *value_ref;

            match self.send_by_client_id(
                &client_id,
                WriteRequest {
                    packet: pkt.clone(),
                    mqtt_id: mqtt_id.to_string(),
                },
            ) {
                Ok(_) => {
                    debug!("send suceed!! {:?}", mqtt_id);
                    Ok(())
                }
                Err(e) => match e {
                    TrySendError::Closed(_) => {
                        let _ = self.by_mqtt_id.remove(mqtt_id);
                        self.by_client_mqtt.remove(&client_id);
                        self.unregister_client_id(client_id);
                        if let Some(mut q) = self.queue_by_mqtt_id.get_mut(mqtt_id) {
                            q.push_front(pkt);
                        } else {
                            let mut new_q = VecDeque::new();
                            new_q.push_back(pkt);
                            self.queue_by_mqtt_id.insert(mqtt_id.clone(), new_q);
                        }
                        return Ok(());
                    }
                    TrySendError::Full(_orig) => {
                        // バッファ満杯ならキューに入れておく（push_back で順序維持）
                        if let Some(mut q) = self.queue_by_mqtt_id.get_mut(mqtt_id) {
                            q.push_back(pkt);
                        } else {
                            let mut new_q = VecDeque::new();
                            new_q.push_back(pkt);
                            self.queue_by_mqtt_id.insert(mqtt_id.clone(), new_q);
                        }
                        return Ok(());
                    }
                },
            }
        } else {
            trace!(
                "cannot send anything -> queueing packet for mqtt_id {}",
                mqtt_id
            );
            // mqtt_id に登録が無い場合でもキューに保存する
            if let Some(mut q) = self.queue_by_mqtt_id.get_mut(mqtt_id) {
                q.push_back(pkt);
            } else {
                let mut new_q = VecDeque::new();
                new_q.push_back(pkt);
                self.queue_by_mqtt_id.insert(mqtt_id.clone(), new_q);
            }
            Ok(())
        }
    }

    pub fn register_mqtt_id(&self, mqtt_id: String, client_id: Uuid) {
        trace!("register_mqtt_id {:?} {:?}", mqtt_id, client_id);
        if let Some((_, old_client_id)) = self.by_mqtt_id.remove(&mqtt_id) {
            debug!("removing old client {:?}", old_client_id);
            self.by_client_mqtt.remove(&old_client_id);
            self.unregister_client_id(old_client_id);
        }

        self.by_mqtt_id.insert(mqtt_id.clone(), client_id);
        self.by_client_mqtt.insert(client_id, mqtt_id);
    }
    pub fn unregister_mqtt_id(&self, mqtt_id: String) {
        if let Some((_, client_id)) = self.by_mqtt_id.remove(&mqtt_id) {
            self.by_client_mqtt.remove(&client_id);
            self.unregister_client_id(client_id);
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
