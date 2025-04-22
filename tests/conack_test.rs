use aqua::Connection;
use aqua::{request, response};
use aqua::{ConnackError, ConnackResponse};
use mqtt_coder::mqtt::{self, Connack, ControlPacket, Pingresp, ProtocolVersion};
use paho_mqtt;
use std::convert::Infallible;
use std::{net::SocketAddr, time::Duration};
use tokio;
use tokio::net::TcpListener;
use tower::service_fn;

#[tokio::test]
async fn async_test() -> Result<(), Box<dyn std::error::Error>> {
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
                    // ここでは CONNECT 以外はデフォルトレスポンスを返す
                    println!("(normal) Received request");
                    match req.body {
                        ControlPacket::PINGREQ(_ping) => {
                            return Ok(response::Response::new(ControlPacket::PINGRESP(
                                Pingresp {},
                            )))
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
                                remaining_length: 0,
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
        /*
        let make_service = service_fn(|incoming: request::IncomingStream| async move {
            println!("New connection from: {:?}", incoming.addr);
            Ok::<_, Infallible>(service_fn(|req: request::Request<ControlPacket>| {
                Box::pin(async move {
                    dbg!("Check request");
                    match req.body {
                        ControlPacket::CONNECT(connect_data) => {
                            println!("Received CONNECT: {:?}", connect_data);
                            let connack_packet = ControlPacket::CONNACK(Default::default());
                            let response = response::Response::new(connack_packet);
                            Ok::<_, std::io::Error>(response)
                        }
                        other => {
                            println!("Received non-CONNECT packet: {:?}", other);
                            Ok::<_, std::io::Error>(response::Response::default())
                        }
                    }
                })
            }))
        });
        */
        // `serve` を使ってサーバーを起動
        aqua::serve(listener, make_service, make_connect_service)
            .await
            .unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    {
        let create_ops = paho_mqtt::CreateOptionsBuilder::new()
            .server_uri("tcp://".to_owned() + str_addr)
            .client_id("test-client-paho")
            .finalize();
        let client = paho_mqtt::AsyncClient::new(create_ops).unwrap();
        let conn_opts = paho_mqtt::ConnectOptionsBuilder::new()
            .keep_alive_interval(Duration::from_secs(5))
            .clean_session(true)
            .finalize();
        println!("Connecting to MQTT broker...");

        let ret = client.connect(conn_opts).await;
        if let Err(e) = ret {
            dbg!(e);
        } else {
            println!("Success!!");
        }

        tokio::time::sleep(Duration::from_secs(10)).await;
        //client.disconnect().await.unwrap();
    }
    Ok(())
}
