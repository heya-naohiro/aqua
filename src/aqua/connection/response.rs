use mqtt_coder::mqtt::ControlPacket;
use uuid::Uuid;

#[derive(Default)]
pub struct Response {
    pub packet: ControlPacket,
    client_id: Uuid,
}

impl Response {
    pub fn new(packet: ControlPacket) -> Self {
        let id = Uuid::new_v4();
        return Self {
            packet,
            client_id: id,
        };
    }
}
