use std::net::SocketAddr;

use mqtt_coder::mqtt::MqttPacket;
use tokio::net::TcpStream;

#[derive(Clone, Default)]
pub struct Request<T> {
    //    client_id: String, [TODO]
    pub body: T,
}

impl<T> Request<T> {
    pub fn new(body: T) -> Self {
        Self { body: body }
    }
}

pub struct IncomingStream {
    pub tcp_stream: TcpStream,
    pub addr: SocketAddr,
}

pub struct IncomingMqtt {
    pub packet: Box<dyn MqttPacket>,
}
