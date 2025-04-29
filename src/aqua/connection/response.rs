use mqtt_coder::mqtt::ControlPacket;
use uuid::Uuid;

#[derive(Default, Debug)]
pub struct Response {
    pub packet: ControlPacket,
}

impl Response {
    pub fn new(packet: ControlPacket) -> Self {
        return Self { packet };
    }
}

/* Connack Response
#[derive(Default)]
pub struct ConnackResponse {
    pub packet: ControlPacket,
    client_id: Uuid,
}

*/
