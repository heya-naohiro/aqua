use aqua::SESSION_MANAGER;
use aqua::{request, response};
use aqua::{ConnackError, ConnackResponse};
use mini_redis::client;
use mqtt_coder::mqtt::{
    self, Connack, ControlPacket, Pingresp, ProtocolVersion, Suback, SubackReasonCode,
    SubscribeOption,
};
use paho_mqtt::topic;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio;
use tokio::net::TcpListener;
use topic_manager::TopicManager;
use tower::service_fn;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let str_addr = "127.0.0.1:1883";
    let addr = str_addr.parse::<SocketAddr>().unwrap();
    let topic_mgr = Arc::new(TopicManager::new());
    let make_service = {
        let topic_mgr = Arc::clone(&topic_mgr);
        service_fn(move |incoming: request::IncomingStream| {
            let topic_mgr = Arc::clone(&topic_mgr);
            let peer = incoming.addr;
            let client_id_opt = incoming.client_id;
            async move {
                println!("(normal) New connection from: {:?}", peer);
                Ok::<_, Infallible>(service_fn(move |req: request::Request<ControlPacket>| {
                    let topic_mgr = Arc::clone(&topic_mgr);
                    let client_id_opt = client_id_opt;
                    Box::pin(async move {
                        let client_id: Uuid = if let Some(client_id) = client_id_opt {
                            client_id
                        } else {
                            return Err(mqtt::MqttError::Unexpected);
                        };
                        match req.body {
                            ControlPacket::PINGREQ(_ping) => {
                                return Ok(response::Response::new(ControlPacket::PINGRESP(
                                    Pingresp {},
                                )))
                            }
                            ControlPacket::SUBSCRIBE(subpacket) => {
                                let mut success_codes = vec![];
                                for (filter, suboption) in subpacket.topic_filters {
                                    topic_mgr.register(filter.value(), client_id, &suboption);
                                    success_codes.push(SubackReasonCode::from(suboption.qos));
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
                                let topic_mgr = Arc::clone(&topic_mgr);
                                let pubpacket = pubpacket.clone();
                                tokio::spawn(async move {
                                    if let Some(topic_name) = pubpacket.topic_name.clone() {
                                        let subed_clients =
                                            topic_mgr.subed_id(topic_name.value().to_string());
                                        for (sub_id, _optioin) in subed_clients {
                                            let result = SESSION_MANAGER.send(
                                                &sub_id,
                                                ControlPacket::PUBLISH(pubpacket.clone()),
                                            );
                                            if let Ok(()) = result {
                                                println!("Delivering to {:?}", sub_id);
                                            } else {
                                                println!("Error {:?}", sub_id);
                                            }
                                        }
                                    } else {
                                        print!("Pubpacket topic name not found ")
                                    }
                                });
                            }
                            _ => {}
                        }
                        Ok(response::Response::default())
                    })
                }))
            }
        })
    };
    let make_connect_service = service_fn(|incoming: request::IncomingStream| async move {
        println!("(connect) New connection from: {:?}", incoming.addr);
        Ok::<_, Infallible>(service_fn(|req: request::Request<ControlPacket>| {
            Box::pin(async move {
                println!("(connect) Processing request");
                match req.body {
                    ControlPacket::CONNECT(_connect_data) => {
                        // CONNECT パケットを受け取ったとき
                        let connack_data = Connack {
                            session_present: false,
                            connect_reason: mqtt::ConnackReason::Success,
                            connack_properties: None,
                            version: ProtocolVersion::new(0x04),
                        };
                        let connack_response = ConnackResponse::from(connack_data);
                        dbg!("(connect) Connack response, ", &connack_response);
                        Ok(connack_response)
                    }
                    _ => {
                        println!("(connect) Received non-CONNECT packet");
                        Ok(ConnackResponse::default())
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
    use uuid::Uuid;

    pub struct TopicManager {
        topic_map: DashMap<String, Vec<(Uuid, SubscribeOption)>>,
    }

    impl TopicManager {
        pub fn new() -> Self {
            let topic_map = DashMap::new();
            Self { topic_map }
        }

        pub fn register(&self, topic_filter: String, client_id: Uuid, option: &SubscribeOption) {
            self.topic_map
                .entry(topic_filter)
                .or_default()
                .push((client_id, option.clone()));
        }

        pub fn unregister(&self, topic_filter: String, client_id: Uuid) {
            if let Some(mut v) = self.topic_map.get_mut(&topic_filter) {
                v.retain(|x| x.0 != client_id);
            }
        }

        pub fn subed_id(&self, topic: String) -> Vec<(Uuid, SubscribeOption)> {
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
            let filter_elms: Vec<String> = topic_filter.split("/").map(|s| s.to_string()).collect();
            let mut filter_index = 0;
            for te in topic.split('/') {
                if filter_elms[filter_index] == "#" {
                    return true;
                }
                if te == filter_elms[filter_index] || filter_elms[filter_index] == "+" {
                    filter_index = filter_index + 1;
                } else {
                    return false;
                }
            }
            return true;
        }
    }
}
