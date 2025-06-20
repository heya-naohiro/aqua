use aqua::session_manager;
use aqua::ConnackError;
use aqua::ConnackResponse;
use aqua::SESSION_MANAGER;
use aqua::{request, response};
use mqtt_coder::mqtt::Puback;
use mqtt_coder::mqtt::{
    self, Connack, ControlPacket, MqttError, Pingresp, ProtocolVersion, Suback, SubackReasonCode,
};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio;
use tokio::net::TcpListener;
use topic_manager::TopicManager;
use tower::service_fn;
use tracing::trace;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE) // ここが重要！
        .init();
    let str_addr = "127.0.0.1:1883";
    let addr = str_addr.parse::<SocketAddr>().unwrap();
    let topic_mgr = Arc::new(TopicManager::new());

    let make_service = {
        service_fn(move |incoming: request::IncomingStream| {
            let topic_mgr = Arc::clone(&topic_mgr);
            let mqtt_id_lock = incoming.mqtt_id.clone();
            let peer = incoming.addr;
            async move {
                Ok::<_, Infallible>(service_fn(move |req: request::Request<ControlPacket>| {
                    trace!("(normal) {:?} {:?}", peer, req.body);
                    let mqtt_id_lock = mqtt_id_lock.clone();
                    let topic_mgr = Arc::clone(&topic_mgr);

                    Box::pin(async move {
                        match req.body {
                            ControlPacket::DISCONNECT(_disconnect) => {
                                trace!("Disconnect");
                                let mqtt_id_guard = mqtt_id_lock.read().await;
                                let mqtt_id = mqtt_id_guard.clone();
                                drop(mqtt_id_guard);
                                if !mqtt_id.is_empty() {
                                    SESSION_MANAGER.unregister_mqtt_id(mqtt_id.clone());
                                    trace!(
                                        "DISCONNECT: MQTT ID '{}' unregistered",
                                        mqtt_id.clone()
                                    );
                                } else {
                                    trace!("DISCONNECT: MQTT ID was empty; nothing to unregister");
                                }
                                Ok(response::Response::new(ControlPacket::NOOPERATION))
                            }
                            ControlPacket::PINGREQ(_ping) => {
                                trace!("pingreq");
                                return Ok::<response::Response, MqttError>(
                                    response::Response::new(ControlPacket::PINGRESP(Pingresp {})),
                                );
                            }
                            ControlPacket::SUBSCRIBE(subpacket) => {
                                trace!("subscribe");
                                let mut success_codes = vec![];
                                let guard = mqtt_id_lock.read().await;
                                let mqtt_id: String = guard.clone();
                                if mqtt_id != "".to_string() {
                                    for (filter, suboption) in subpacket.topic_filters {
                                        trace!("Subscribe Done!! ");
                                        topic_mgr.register(filter.value(), &mqtt_id, &suboption);
                                        success_codes.push(SubackReasonCode::from(suboption.qos));
                                    }
                                }

                                return Ok(response::Response::new(ControlPacket::SUBACK({
                                    Suback {
                                        packet_id: subpacket.packet_id,
                                        suback_properties: None,
                                        reason_codes: success_codes,
                                        protocol_version: ProtocolVersion::new(0x05),
                                    }
                                })));
                            }
                            ControlPacket::PUBLISH(pubpacket) => {
                                trace!("publish");
                                let topic_mgr = Arc::clone(&topic_mgr);
                                let pubpacket_clone_for_spawn = pubpacket.clone();
                                let qos = pubpacket.qos;
                                let packet_id = pubpacket.packet_id.clone();
                                tokio::spawn(async move {
                                    let subed_clients = topic_mgr.subed_id(
                                        pubpacket_clone_for_spawn
                                            .topic_name
                                            .clone()
                                            .value()
                                            .to_string(),
                                    );
                                    for (sub_id, _option) in subed_clients {
                                        let result = SESSION_MANAGER.send_by_mqtt_id(
                                            &sub_id,
                                            ControlPacket::PUBLISH(
                                                pubpacket_clone_for_spawn.clone(),
                                            ),
                                        );
                                        if let Ok(()) = result {
                                            println!("Delivering to {:?}", sub_id);
                                        } else {
                                            println!("Error {:?}", sub_id);
                                        }
                                    }
                                });
                                // [TODO] need pub id check except QoS0
                                match qos {
                                    mqtt::QoS::QoS0 => {
                                        return Ok(response::Response::new(
                                            ControlPacket::NOOPERATION,
                                        ));
                                    }
                                    mqtt::QoS::QoS1 => {
                                        return Ok(response::Response::new(ControlPacket::PUBACK(
                                            Puback {
                                                reason_code: None,
                                                packet_id: pubpacket.packet_id.unwrap(),
                                                puback_properties: None,
                                                protocol_version: pubpacket.protocol_version,
                                            },
                                        )));
                                    }
                                    mqtt::QoS::QoS2 => {
                                        let v = pubpacket.protocol_version.clone();
                                        let id = packet_id.unwrap().clone();
                                        SESSION_MANAGER
                                            .add_staging_packet(pubpacket, mqtt::QoS::QoS2);
                                        return Ok(response::Response::new(ControlPacket::PUBREC(
                                            mqtt::Pubrec {
                                                packet_id: id,
                                                reason_code: Some(mqtt::PubrecReasonCode::Success),
                                                pubrec_properties: None,
                                                protocol_version: v,
                                            },
                                        )));
                                    }
                                }
                            }
                            ControlPacket::CONNECT(_) => {
                                trace!(
                                    "Protocol error: CONNECT received after session established"
                                );
                                return Err(mqtt::MqttError::ProtocolViolation);
                            }

                            ControlPacket::PUBREL(pubrel_packet) => {
                                trace!("pubrel!!!!!!!!!!");
                                // PUBREL受信時の処理を追加する必要があります
                                let packet_id = pubrel_packet.packet_id;
                                // QoS2ステージングメッセージから該当メッセージを取り出す処理も必要（実装済みなら呼ぶ）
                                //SESSION_MANAGER.remove_staging_packet(packet_id);

                                return Ok(response::Response::new(ControlPacket::PUBCOMP(
                                    mqtt::Pubcomp {
                                        packet_id,
                                        pubcomp_reason: Some(mqtt::PubcompReasonCode::Success),
                                        pubcomp_properties: None,
                                        protocol_version: pubrel_packet.protocol_version,
                                    },
                                )));
                            }
                            other => {
                                trace!("other {:?}", other);
                                return Ok(response::Response::new(ControlPacket::NOOPERATION));
                                //return Ok(response::Response::default(response::Response::new());
                            }
                        }
                    })
                }))
            }
        })
    };
    let make_connect_service = service_fn(move |incoming: request::IncomingStream| async move {
        let mqtt_id_lock = incoming.mqtt_id.clone();
        Ok::<_, Infallible>(service_fn(move |req: request::Request<ControlPacket>| {
            let mqtt_id_lock = mqtt_id_lock.clone();
            Box::pin(async move {
                println!("(connect) Processing request");
                match req.body {
                    ControlPacket::CONNECT(connect_data) => {
                        let connack_data = Connack {
                            session_present: false,
                            connect_reason: mqtt::ConnackReason::Success,
                            connack_properties: None,
                            version: ProtocolVersion::new(0x04),
                        };
                        let client_id: String = connect_data.client_id.clone().into_inner();
                        {
                            let mut guard = mqtt_id_lock.write().await;
                            *guard = client_id.clone();
                        }
                        trace!(
                            "========== register {:?}, {:?}",
                            client_id.clone(),
                            incoming.client_id.clone()
                        );
                        SESSION_MANAGER
                            .register_mqtt_id(client_id.clone(), incoming.client_id.clone());
                        let connack_response = ConnackResponse::from(connack_data);
                        trace!("(connect) Connack response");
                        Ok(connack_response)
                    }
                    _ => {
                        println!("(connect) Received non-CONNECT packet");
                        Err(ConnackError {
                            reason_code: mqtt::ConnackReason::ProtocolError, // 例として ProtocolError
                        })
                    }
                }
            })
        }))
    });
    let listener = TcpListener::bind(addr).await?;
    aqua::serve(listener, make_service, make_connect_service).await?;
    Ok(())
}

pub mod topic_manager {
    use dashmap::DashMap;
    use mqtt_coder::mqtt::SubscribeOption;

    pub struct TopicManager {
        topic_map: DashMap<String, Vec<(String, SubscribeOption)>>,
    }

    impl TopicManager {
        pub fn new() -> Self {
            let topic_map = DashMap::new();
            Self { topic_map }
        }

        pub fn register(&self, topic_filter: String, client_id: &String, option: &SubscribeOption) {
            self.topic_map
                .entry(topic_filter)
                .or_default()
                .push((client_id.clone(), option.clone()));
        }

        pub fn unregister(&self, topic_filter: String, client_id: String) {
            if let Some(mut v) = self.topic_map.get_mut(&topic_filter) {
                v.retain(|x| x.0 != client_id);
            }
        }

        pub fn subed_id(&self, topic: String) -> Vec<(String, SubscribeOption)> {
            let topic_filters: Vec<_> = self
                .topic_map
                .iter()
                .map(|entry| entry.key().clone())
                .collect();
            let mut ret = vec![];
            for topic_filter in topic_filters {
                if self.is_match(&topic_filter, &topic) {
                    if let Some(v2) = self.topic_map.get(&topic) {
                        ret.extend(v2.clone());
                    }
                }
            }
            return ret;
        }

        fn is_match(&self, topic_filter: &String, topic: &String) -> bool {
            let filter_segment: Vec<&str> = topic_filter.split('/').collect();
            let topic_segment: Vec<&str> = topic.split('/').collect();

            let mut filter_index = 0;
            let mut topic_index = 0;
            while filter_index < filter_segment.len() {
                match filter_segment[filter_index] {
                    "#" => {
                        // should be last
                        return filter_index == filter_segment.len() - 1;
                    }
                    "+" => {
                        if topic_index >= topic_segment.len() {
                            return false;
                        }
                        filter_index += 1;
                        topic_index += 1;
                    }
                    filter
                        if topic_index < topic_segment.len()
                            && filter == topic_segment[topic_index] =>
                    {
                        filter_index += 1;
                        topic_index += 1;
                    }
                    _ => {
                        return false;
                    }
                }
            }
            // have been comsumed all segments
            topic_index == topic_segment.len()
        }
    }
}
