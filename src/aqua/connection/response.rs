use mqtt_coder::mqtt::ControlPacket;

pub struct Response {
    pub packet: ControlPacket,
    client_id: usize,
}
