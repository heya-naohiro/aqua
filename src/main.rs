use mqtt_coder::mqtt::{
    self, ControlPacket, Pingresp, ProtocolVersion, Puback, Pubrec, QoS, Suback, SubackReasonCode,
};
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::service_fn;
mod aqua;
use aqua::connection::connack_response::ConnackResponse;
use aqua::connection::request::Request;
use aqua::connection::response::Response;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TCP リスナーを作成
    let addr = "127.0.0.1:1883".parse::<SocketAddr>().unwrap();
    let listener = TcpListener::bind(addr).await?;
    println!("MQTT Server listening on {}", addr);

    let make_service = service_fn(
        |incoming: aqua::connection::request::IncomingStream| async move {
            println!("(normal) New connection from: {:?}", incoming.addr);
            Ok::<_, Infallible>(service_fn(|req: Request<ControlPacket>| {
                Box::pin(async move {
                    println!("(normal) Received request: {:?}", req.body);
                    match req.body {
                        // PING要求に対する応答
                        ControlPacket::PINGREQ(_ping) => {
                            println!("Received PINGREQ, responding with PINGRESP");
                            return Ok(Response::new(ControlPacket::PINGRESP(Pingresp {})));
                        }
                        // SUBSCRIBE要求に対する応答
                        ControlPacket::SUBSCRIBE(subscribe) => {
                            println!("Received SUBSCRIBE, responding with SUBACK");
                            let packet_id = subscribe.packet_id;
                            let mut reason_codes = Vec::new();

                            // 各トピックフィルターに対して応答コードを設定
                            for (topic_filter, option) in subscribe.topic_filters {
                                println!("  Topic filter: {:?}", topic_filter);
                                // オプションがある場合はQoSを取得、なければデフォルトでQoS0
                                let qos = if let Some(opt) = option {
                                    opt.qos
                                } else {
                                    QoS::QoS0
                                };

                                // QoSに応じた応答コードを追加
                                match qos {
                                    QoS::QoS0 => reason_codes.push(SubackReasonCode::GrantedQoS0),
                                    QoS::QoS1 => reason_codes.push(SubackReasonCode::GrantedQoS1),
                                    QoS::QoS2 => reason_codes.push(SubackReasonCode::GrantedQoS2),
                                }
                            }

                            // SUBACKパケットを作成
                            let suback = Suback {
                                remain_length: 0, // 自動計算される
                                packet_id,
                                suback_properties: None, // MQTT5のみで使用
                                reason_codes,
                                protocol_version: ProtocolVersion(0x04), // MQTT 3.1.1
                            };

                            return Ok(Response::new(ControlPacket::SUBACK(suback)));
                        }
                        // PUBLISH要求に対する応答（QoSに依存）
                        ControlPacket::PUBLISH(publish) => {
                            println!("Received PUBLISH");
                            match publish.qos {
                                QoS::QoS0 => {
                                    // QoS0の場合は応答不要
                                    println!("  QoS0: No acknowledgement required");
                                    return Ok(Response::default());
                                }
                                QoS::QoS1 => {
                                    // QoS1の場合はPUBACKを返す
                                    println!("  QoS1: Responding with PUBACK");
                                    if let Some(packet_id) = publish.packet_id {
                                        let puback = mqtt::Puback {
                                            remaining_length: 0, // 自動計算される
                                            packet_id,
                                            reason_code: None,
                                            puback_properties: None,
                                            protocol_version: ProtocolVersion(0x04), // MQTT 3.1.1
                                        };
                                        return Ok(Response::new(ControlPacket::PUBACK(puback)));
                                    }
                                }
                                QoS::QoS2 => {
                                    // QoS2の場合はPUBRECを返す（第1段階）
                                    println!("  QoS2: Responding with PUBREC");
                                    if let Some(packet_id) = publish.packet_id {
                                        let pubrec = mqtt::Pubrec {
                                            remaining_length: 0, // 自動計算される
                                            packet_id,
                                            reason_code: None,
                                            pubrec_properties: None,
                                            protocol_version: ProtocolVersion(0x04), // MQTT 3.1.1
                                        };
                                        return Ok(Response::new(ControlPacket::PUBREC(pubrec)));
                                    }
                                }
                            }
                            return Ok(Response::default());
                        }
                        // PUBRELに対する応答（QoS2の第2段階）
                        ControlPacket::PUBREL(pubrel) => {
                            println!("Received PUBREL, responding with PUBCOMP");
                            let pubcomp = mqtt::Pubcomp {
                                remaining_length: 0,
                                packet_id: pubrel.packet_id,
                                pubcomp_reason: None,
                                protocol_version: ProtocolVersion(0x04),
                                pubcomp_properties: None,
                            };
                            return Ok(Response::new(ControlPacket::PUBCOMP(pubcomp)));
                        }
                        // UNSUBSCRIBEに対する応答
                        ControlPacket::UNSUBSCRIBE(unsubscribe) => {
                            println!("Received UNSUBSCRIBE, responding with UNSUBACK");
                            let unsuback = mqtt::Unsuback {
                                remaining_length: 0,
                                packet_id: unsubscribe.packet_id,
                                unsuback_properties: None,
                                reason_codes: vec![
                                    mqtt::UnsubackReasonCode::Success;
                                    unsubscribe.topic_filters.len()
                                ],
                                protocol_version: ProtocolVersion(0x04),
                            };
                            return Ok(Response::new(ControlPacket::UNSUBACK(unsuback)));
                        }
                        _ => {
                            println!("Received other packet type");
                            Ok(Response::default())
                        }
                    }
                })
            }))
        },
    );

    // CONNECT処理用サービス
    let make_connect_service = service_fn(
        |incoming: aqua::connection::request::IncomingStream| async move {
            println!("(connect) New connection from: {:?}", incoming.addr);
            Ok::<_, Infallible>(service_fn(|req: Request<ControlPacket>| {
                Box::pin(async move {
                    println!("(connect) Processing request");
                    match req.body {
                        ControlPacket::CONNECT(connect_data) => {
                            // CONNECT パケットを受け取ったとき
                            println!("Received CONNECT: {:?}", connect_data.client_id);
                            let protocol_version = connect_data.protocol_ver;
                            let mut connack_response = ConnackResponse {
                                session_present: false,
                                version: protocol_version,
                                connack_properties: None,
                            };

                            Ok(connack_response)
                        }
                        _ => {
                            println!("(connect) Received non-CONNECT packet");
                            Ok(ConnackResponse::default())
                        }
                    }
                })
            }))
        },
    );

    // `serve` を使ってサーバーを起動
    aqua::serve(listener, make_service, make_connect_service).await
}
