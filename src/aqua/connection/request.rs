use mqtt_decoder::mqtt::ControlPacket;

pub struct Request {
    packet: ControlPacket,
    client_id: usize,
}
