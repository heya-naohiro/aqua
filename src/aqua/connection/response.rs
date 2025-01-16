use mqtt_decoder::mqtt::ControlPacket;

pub struct Response {
    packet: ControlPacket,
    client_id: usize,
}
