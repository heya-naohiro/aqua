use std::net::SocketAddr;
use tokio::net::TcpListener;

use aqua::{request, response};
use mqtt_coder::mqtt::ControlPacket;
use std::convert::Infallible;
use tokio;
use tower::service_fn;

#[tokio::test]
async fn connack() {
    let server_handle = tokio::spawn(async {
        let addr = "127.0.0.1:1883".parse::<SocketAddr>().unwrap();
        let listener = TcpListener::bind(addr).await.unwrap();

        let make_service = service_fn(|incoming: request::IncomingStream| async move {
            println!("New connection from: {:?}", incoming.addr);
            Ok::<_, Infallible>(service_fn(|req: request::Request<ControlPacket>| {
                Box::pin(async move {
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

        // `serve` を使ってサーバーを起動
        aqua::serve(listener, make_service).await.unwrap();
    });

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    {
        // [TODO] test, use mqtt library
    }
}
