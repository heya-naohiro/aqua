use std::{net::SocketAddr, sync::Arc};

use mqtt_coder::mqtt::MqttPacket;
use tokio::net::TcpStream;
use uuid::Uuid;

#[derive(Clone, Default, Debug)]
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
    pub tcp_stream: Arc<TcpStream>,
    pub addr: SocketAddr,
    pub client_id: Option<Uuid>,
    pub mqtt_id: Option<String>,
}

pub struct IncomingMqtt {
    pub packet: Box<dyn MqttPacket>,
}
