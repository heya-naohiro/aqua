use aqua::SESSION_MANAGER;
use aqua::{request, response};
use aqua::{ConnackError, ConnackResponse};
use mqtt_coder::mqtt::{
    self, Connack, ControlPacket, Pingresp, ProtocolVersion, Suback, SubackReasonCode,
    SubscribeOption,
};
use paho_mqtt::topic;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio;
use tokio::net::TcpListener;
use topic_manager::TopicManager;
use tower::service_fn;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let str_addr = "127.0.0.1:1883";
    let addr = str_addr.parse::<SocketAddr>().unwrap();
    let topic_mgr = TopicManager::new();
    dbg!("Hello, async_test world");
    tokio::spawn(async move {
        let listener = TcpListener::bind(addr).await.unwrap();
        let make_service = service_fn(|incoming: request::IncomingStream| async move {
            println!("(normal) New connection from: {:?}", incoming.addr);
            Ok::<_, Infallible>(service_fn(|req: request::Request<ControlPacket>| {
                Box::pin(async move {
                    /* client id error */
                    let client_id = if let Some(client_id) = incoming.client_id {
                        client_id
                    } else {
                        return Err(mqtt::MqttError::Unexpected);
                    };

                    println!("(normal) Received request");
                    match req.body {
                        ControlPacket::PINGREQ(_ping) => {
                            return Ok(response::Response::new(ControlPacket::PINGRESP(
                                Pingresp {},
                            )))
                        }
                        ControlPacket::SUBSCRIBE(subpacket) => {
                            let mut success_codes = vec![];
                            for (filter, suboption) in subpacket.topic_filters {
                                topic_mgr.register(filter.value(), client_id, suboption);
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
                        ControlPacket::PUBLISH(_pubpacket) => {
                            /* [TODO] topic implement -> clients */
                            /* [TODO] another threads, because not need to sync */

                            // SESSION_MANAGER.send(client_id, pkt)
                        }
                        _ => {}
                    }
                    Ok::<_, mqtt::MqttError>(response::Response::default())
                })
            }))
        });
        // CONNECT 用サービス
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
        // `serve` を使ってサーバーを起動
        aqua::serve(listener, make_service, make_connect_service)
            .await
            .unwrap();
    });

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

        pub fn register(&mut self, topic_filter: String, client_id: Uuid, option: SubscribeOption) {
            self.topic_map
                .entry(topic_filter)
                .or_default()
                .push((client_id, option));
        }

        pub fn unregister(&mut self, topic_filter: String, client_id: Uuid) {
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
