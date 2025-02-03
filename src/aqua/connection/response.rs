use mqtt_decoder::mqtt::ControlPacket;

pub struct Response {
    pub packet: ControlPacket,
    client_id: usize,
}
