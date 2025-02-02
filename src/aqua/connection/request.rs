use std::net::SocketAddr;

use mqtt_decoder::mqtt::MqttPacket;
use tokio::net::TcpStream;

pub struct Request<T> {
    client_id: String,
    body: T,
}

pub struct IncomingStream {
    pub tcp_stream: TcpStream,
    pub addr: SocketAddr,
}

pub struct IncomingMqtt {
    pub packet: Box<dyn MqttPacket>,
}
