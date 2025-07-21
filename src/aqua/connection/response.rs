use mqtt_coder::mqtt::ControlPacket;
use uuid::Uuid;

#[derive(Default, Debug)]
pub struct Response {
    pub packets: Vec<ControlPacket>,
}

impl Response {
    pub fn new(packet: ControlPacket) -> Self {
        return Self {
            packets: vec![packet],
        };
    }
    pub fn new_packets(packets: Vec<ControlPacket>) -> Self {
        return Self { packets };
    }
}

/* Connack Response
#[derive(Default)]
pub struct ConnackResponse {
    pub packet: ControlPacket,
    client_id: Uuid,
}

*/
