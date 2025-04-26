use aqua::Connection;
use aqua::{request, response};
use aqua::{ConnackError, ConnackResponse};
use dashmap::DashMap;
use mqtt_coder::mqtt::{
    self, Connack, ControlPacket, Pingresp, ProtocolVersion, Suback, SubackReasonCode,
};
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio;
use tokio::net::TcpListener;
use tower::service_fn;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();
    let str_addr = "127.0.0.1:1883";
    let addr = str_addr.parse::<SocketAddr>().unwrap();
    dbg!("Hello, async_test world");
    tokio::spawn(async move {
        let listener = TcpListener::bind(addr).await.unwrap();
        let make_service = service_fn(|incoming: request::IncomingStream| async move {
            println!("(normal) New connection from: {:?}", incoming.addr);
            Ok::<_, Infallible>(service_fn(|req: request::Request<ControlPacket>| {
                Box::pin(async move {
                    println!("(normal) Received request");
                    match req.body {
                        ControlPacket::PINGREQ(_ping) => {
                            return Ok(response::Response::new(ControlPacket::PINGRESP(
                                Pingresp {},
                            )))
                        }
                        ControlPacket::SUBSCRIBE(subpacket) => {
                            let mut success_codes = vec![];
                            for (_filters, suboption) in subpacket.topic_filters {
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
                        _ => {}
                    }
                    Ok::<_, std::io::Error>(response::Response::default())
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
