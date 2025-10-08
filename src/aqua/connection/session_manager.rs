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
use tracing::{debug, info, trace};
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
    // ここを変更: (mqtt_id, packet_id) をキーにする
    qos_tmp: Arc<DashMap<(String, u16), (Publish, QoS)>>,

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
    pub fn remove_all_for_mqtt_id(&self, mqtt_id: &str) {
        // 一旦キーだけ collect してから remove する
        let keys: Vec<(String, u16)> = self
            .qos_tmp
            .iter()
            .filter(|kv| kv.key().0 == mqtt_id)
            .map(|kv| kv.key().clone())
            .collect();

        for key in keys {
            self.qos_tmp.remove(&key);
        }
    }
    // for re-send, for QoS2
    pub fn add_staging_packet(&self, mqtt_id: &str, pkt: Publish, qos: QoS) {
        info!("add_staging_packet");
        if let Some(ref id) = pkt.packet_id {
            self.qos_tmp
                .insert((mqtt_id.to_string(), id.value()), (pkt, qos));
        }
    }
    pub fn fetch_packet(&self, mqtt_id: &str, pid: PacketId) -> Result<Publish, MqttError> {
        info!("fetch_packet");
        if let Some(entry) = self.qos_tmp.get(&(mqtt_id.to_string(), pid.value())) {
            let (publish, _) = entry.value();
            Ok(publish.clone())
        } else {
            Err(MqttError::Unexpected)
        }
    }

    pub fn commit_packet(&self, mqtt_id: &str, pid: PacketId) -> Result<(), MqttError> {
        info!("commit_packet");
        if self
            .qos_tmp
            .remove(&(mqtt_id.to_string(), pid.value()))
            .is_some()
        {
            Ok(())
        } else {
            Err(MqttError::Unexpected)
        }
    }
    pub fn drain_packets_by_mqtt_id(&self, mqtt_id: &str) -> Vec<(PacketId, Publish, QoS)> {
        info!("drain_packets_by_mqtt_id");
        let mut removed = Vec::new();
        // DashMap は直接 filter_drain できないので collect → remove
        let keys: Vec<(String, u16)> = self
            .qos_tmp
            .iter()
            .filter(|kv| kv.key().0 == mqtt_id)
            .map(|kv| kv.key().clone())
            .collect();

        for key in keys {
            if let Some((_, (publish, qos))) = self.qos_tmp.remove(&key) {
                removed.push((PacketId::new(key.1), publish, qos));
            }
        }

        removed
    }
    pub fn replay_inflight(&self, mqtt_id: &str) -> Vec<ControlPacket> {
        info!("replay_inflight");
        let mut packets = Vec::new();

        // その mqtt_id に紐づく未完了を収集
        let inflights: Vec<((String, u16), (Publish, QoS))> = self
            .qos_tmp
            .iter()
            .filter(|kv| kv.key().0 == mqtt_id)
            .map(|kv| (kv.key().clone(), kv.value().clone()))
            .collect();

        for ((_mid, _pid), (mut publish, qos)) in inflights {
            match qos {
                QoS::QoS1 => {
                    // DUP フラグを立てて再送
                    publish.dup = mqtt::Dup::new(true);
                    packets.push(ControlPacket::PUBLISH(publish));
                }
                QoS::QoS2 => {
                    // QoS2 の場合、セッション状態によって分岐するが、
                    // とりあえず DUP=1 の PUBLISH を再送
                    publish.dup = mqtt::Dup::new(true);
                    packets.push(ControlPacket::PUBLISH(publish));
                }
                QoS::QoS0 => {
                    // QoS0 は保持不要（ここには入らないはず）
                }
            }
        }

        packets
    }

    pub fn register_client_id(&self, client_id: Uuid, outbound: Outbound) {
        info!("register_client_id {:?}", client_id);
        self.by_client_id.insert(client_id, outbound);
    }
    pub fn unregister_client_id(&self, client_id: Uuid) {
        trace!("unregister_client_id {:?}", client_id);
        if let Some((_, outbound)) = self.by_client_id.remove(&client_id) {
            drop(outbound.tx);
        }
        self.by_client_id.remove(&client_id);
    }

    // queued if clean session is false
    pub fn send_by_client_id(
        &self,
        client_id: &Uuid,
        req: WriteRequest,
    ) -> Result<(), TrySendError<WriteRequest>> {
        info!("sent_by_client_id {:?}", client_id);
        if let Some(outbound) = self.by_client_id.get(client_id) {
            outbound.send(req)
        } else {
            error!("TrySendError Closed!!!!!!");
            Err(TrySendError::Closed(req))
        }
    }

    pub fn send_by_mqtt_id_qos1(
        &self,
        mqtt_id: &String,
        pkt: ControlPacket,
    ) -> Result<(), TrySendError<ControlPacket>> {
        // QoS1のために保存する、終わったら削除する。送信できなかった際に
        match pkt.clone() {
            ControlPacket::PUBLISH(publish) => {
                self.add_staging_packet(mqtt_id, publish, QoS::QoS1);
            }
            _ => {
                // ignore
                error!("ignore, not publish packet")
            }
        }
        self.send_by_mqtt_id(mqtt_id, pkt)
    }

    pub fn send_by_mqtt_id_qos2(
        &self,
        mqtt_id: &String,
        pkt: ControlPacket,
    ) -> Result<(), TrySendError<ControlPacket>> {
        // QoS1のために保存する、終わったら削除する。送信できなかった際に
        match pkt.clone() {
            ControlPacket::PUBLISH(publish) => {
                self.add_staging_packet(mqtt_id, publish, QoS::QoS2);
            }
            _ => {
                // ignore
                error!("ignore, not publish packet")
            }
        }
        self.send_by_mqtt_id(mqtt_id, pkt)
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
                        return Ok(());
                    }
                    TrySendError::Full(_orig) => {
                        // バッファ満杯ならキューに入れておく（push_back で順序維持）
                        error!("send error");
                        return Ok(());
                    }
                },
            }
        } else {
            error!(
                "cannot send anything -> queueing packet for mqtt_id {}",
                mqtt_id
            );
            Ok(())
        }
    }

    pub fn register_mqtt_id(&self, mqtt_id: String, client_id: Uuid) {
        info!("register_mqtt_id {:?} {:?}", mqtt_id, client_id);
        if let Some((_, old_client_id)) = self.by_mqtt_id.remove(&mqtt_id) {
            info!("removing old client {:?}", old_client_id);
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
