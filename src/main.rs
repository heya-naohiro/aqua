use mqtt_coder::mqtt::ControlPacket;
use std::convert::Infallible;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower::service_fn;
mod aqua;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    // TCP リスナーを作成
    let addr = "127.0.0.1:1883".parse::<SocketAddr>().unwrap();
    let listener = TcpListener::bind(addr).await?;

    let make_service = service_fn(
        |incoming: aqua::connection::request::IncomingStream| async move {
            println!("New connection from: {:?}", incoming.addr);
            Ok::<_, Infallible>(service_fn(
                |req: aqua::connection::request::Request<ControlPacket>| {
                    Box::pin(async move {
                        match req.body {
                            ControlPacket::CONNECT(connect_data) => {
                                println!("Received CONNECT: {:?}", connect_data);
                                let connack_packet = ControlPacket::CONNACK(Default::default());
                                let response =
                                    aqua::connection::response::Response::new(connack_packet);
                                Ok::<_, std::io::Error>(response)
                            }
                            other => {
                                println!("Received non-CONNECT packet: {:?}", other);
                                Ok::<_, std::io::Error>(
                                    aqua::connection::response::Response::default(),
                                )
                            }
                        }
                    })
                },
            ))
        },
    );

    // `serve` を使ってサーバーを起動
    aqua::serve(listener, make_service).await
}
