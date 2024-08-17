use std::u16;

pub enum ControlPacket {
    CONNECT(Connect),
    DISCONNECT(Disconnect),
    /*
    CONNACK(Conanck),
    PUBLISH(Publish),
    PUBACK(Puback),
    PUBREC(Pubrec),
    PUBREL(Pubrel),
    PUBCOMP(Pubcomp),
    SUBSCRIBE(Subscribe),
    SUBACK(Suback),
    PINGREQ(Pingreq),
    PINGRESP(Pingresp),
    */
}

pub struct MqttPacket {
    remaining_length: usize,
    control_packet: ControlPacket,
}

pub struct Connect {
    pub protocol_ver: ProtocolVersion,
    pub clean_session: bool,
    pub will: bool,
    pub will_qos: u8,
    pub will_retain: bool,
    pub user_password_flag: bool,
    pub user_name_flag: bool,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub keepalive_timer: u16,
    pub properties: ConnectProperties,
}

pub struct ConnectProperties {
    session_expiry_interval: u32,
    receive_maximum: u16,
    maximum_packet_size: u32,
    topic_alias_maximum: u16,
    request_response_information: bool,
    request_problem_infromation: bool,
    user_properties: Vec<(String, String)>,
    authentication_method: Option<String>,
    authentication_data: Option<bytes::Bytes>,
}

impl Default for ConnectProperties {
    fn default() -> Self {
        Self {
            session_expiry_interval: 0,
            receive_maximum: u16::MAX,
            maximum_packet_size: u32::MAX,
            topic_alias_maximum: 0,
            request_response_information: false,
            request_problem_infromation: true,
            user_properties: vec![],
            authentication_method: None,
            authentication_data: None,
        }
    }
}

#[inline]
fn byte_pair_to_u16(b_msb: u8, b_lsb: u8) -> u16 {
    ((b_msb as u16) << 8) + b_lsb as u16
}

#[inline]
pub fn decode_variable_byte(buf: &bytes::Bytes, start_pos: usize) -> (usize, usize) {
    let mut remaining_length: usize = 0;
    for pos in start_pos..=start_pos + 3 {
        remaining_length += (buf[pos] as usize & 0b01111111) << (pos * 7);
        if (buf[pos] & 0b10000000) == 0 {
            return (remaining_length, pos);
        }
    }
    (remaining_length, start_pos + 3)
}

impl Connect {
    // set memebers
    pub fn parse_variable_header(
        &mut self,
        buf: &bytes::Bytes,
        remaining_length: usize,
    ) -> Result<(), anyhow::Error> {
        let protocol_length = (buf[0] as u16) << 8 + buf[1] as u16;
        // MQTT version 5 & 3.1.1 ( MQTT )
        if protocol_length == 4
            && buf[2] == 0b01001101
            && buf[3] == 0b01010001
            && buf[4] == 0b01010100
            && buf[5] == 0b01010100
        {
            if buf[6] == 0b00000101 {
                self.protocol_ver = ProtocolVersion::V5;
            } else if buf[6] == 0b00000100 {
                self.protocol_ver = ProtocolVersion::V3_1_1;
            } else {
                // Error
                return Err(anyhow::anyhow!("Connect Protocol Header Error"));
            }
            self.parse_connect_flag(buf[7]);
            self.keepalive_timer = byte_pair_to_u16(buf[8], buf[9]);
        } else if protocol_length == 6 /* MQTT version 3.1 & 3 ( MQIsdp ) */

            && buf[2] == 0b01001101
            && buf[3] == 0b01010001
            && buf[4] == 0b01001001
            && buf[5] == 0b01110011
            && buf[6] == 0b01100100
            && buf[7] == 0b01110000
        {
            if buf[8] == 0b00000011 {
                self.protocol_ver = ProtocolVersion::V3_1;
            } else {
                // Error
                return Err(anyhow::anyhow!("Connect Protocol Header Error"));
            }
            self.parse_connect_flag(buf[9]);
            self.keepalive_timer = byte_pair_to_u16(buf[10], buf[11]);
        }
        /* CONNECT Properties */
        match self.protocol_ver {
            ProtocolVersion::V5 => {
                let (property_length, end_pos) = decode_variable_byte(buf, 10);
                // property length;
                /* variable  */
                /* TLV format */
                self.parse_tlv_format(buf, end_pos + 1, property_length)?;
            }
            _ => {}
        }

        Ok(())
        // NEXT -> Connect Payload
    }
    #[inline]
    fn parse_connect_flag(&mut self, b: u8) {
        self.clean_session = (b & 0b00000010) == 0b00000010;
        self.will = (b & 0b00000100) == 0b00000100;
        self.will_qos = (b & 0b00011000) >> 3;
        self.will_retain = (b & 0b00100000) == 0b00100000;
        self.user_password_flag = (b & 0b01000000) == 0b01000000;
        self.user_name_flag = (b & 0b10000000) == 0b01000000;
    }
    fn parse_tlv_format(
        &mut self,
        buf: &bytes::Bytes,
        start_pos: usize,
        property_length: usize,
    ) -> Result<(), anyhow::Error> {
        // maximum 21 types properties
        let mut pos = start_pos;

        for _ in 1..=256 {
            // break
            let length = buf[pos + 1] as usize;
            //       8   9  10 11 12 13 | 14 ...
            // ... | x | 4 | p q r s | x  ...
            if pos + 2 + length < buf.len() {
                let value_slice = &buf[pos + 2..pos + 2 + length];
                match buf[pos] {
                    0x11 => {
                        self.properties.session_expiry_interval = u32::from_be_bytes(
                            value_slice
                                .try_into()
                                .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
                        );
                    }
                    0x21 => {
                        self.properties.session_expiry_interval = u32::from_be_bytes(
                            value_slice
                                .try_into()
                                .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
                        );
                    }
                    0x27 => {
                        self.properties.maximum_packet_size = u32::from_be_bytes(
                            value_slice
                                .try_into()
                                .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
                        );
                    }
                    0x22 => {
                        self.properties.topic_alias_maximum = u16::from_be_bytes(
                            value_slice
                                .try_into()
                                .map_err(|_| anyhow::Error::msg("Invalid slice length for u16"))?,
                        )
                    }
                    0x19 => {
                        let tmp = u8::from_be_bytes(
                            value_slice
                                .try_into()
                                .map_err(|_| anyhow::Error::msg("Invalid slice length for u8"))?,
                        );
                        self.properties.request_response_information = match tmp {
                            0b0 => false,
                            0b1 => true,
                            _ => {
                                return Err(anyhow::anyhow!(
                                    "Protocol Error: Request response information"
                                ))
                            }
                        }
                    }
                    0x17 => {
                        let tmp = u8::from_be_bytes(
                            value_slice
                                .try_into()
                                .map_err(|_| anyhow::Error::msg("Invalid slice length for u8"))?,
                        );
                        self.properties.request_problem_infromation = match tmp {
                            0b0 => false,
                            0b1 => true,
                            _ => {
                                return Err(anyhow::anyhow!(
                                    "Protocol Error: Request problem information"
                                ))
                            }
                        }
                    }
                    0x26 => {
                        let mut pos = 0;
                        loop {
                            let name_length = &value_slice[pos..pos + 2];
                            let name_length =
                                u16::from_be_bytes(name_length.try_into().map_err(|_| {
                                    anyhow::Error::msg("Invalid slice length for u8")
                                })?) as usize;
                            let name = match std::str::from_utf8(
                                &value_slice[pos + 2..pos + name_length + 2],
                            ) {
                                Ok(v) => v,
                                Err(_) => {
                                    return Err(anyhow::anyhow!(
                                        "Protocol Error: Invalid user property name, not utf8"
                                    ))
                                }
                            };
                            let value_length =
                                &value_slice[pos + name_length + 2..pos + name_length + 4];
                            let value_length =
                                u16::from_be_bytes(value_length.try_into().map_err(|_| {
                                    anyhow::Error::msg("Invalid slice length for u8")
                                })?) as usize;
                            let value = match std::str::from_utf8(
                                &value_slice[(pos + name_length + 4)
                                    ..(pos + value_length + name_length + 4)],
                            ) {
                                Ok(v) => v,
                                Err(_) => {
                                    return Err(anyhow::anyhow!(
                                        "Protocol Error: Invalid user property name, not utf8"
                                    ))
                                }
                            };
                            self.properties
                                .user_properties
                                .append(&mut vec![(name.to_string(), value.to_string())]);
                            pos = pos + value_length + name_length + 4;
                            if pos == length {
                                break;
                            } else if pos > length {
                                return Err(anyhow::anyhow!(
                                    "Protocol Error: Invalid user property length"
                                ));
                            }
                        }
                    }
                    0x15 => {
                        match self.properties.authentication_method {
                            Some(_) => {
                                return Err(anyhow::anyhow!(
                                    "Protocol Error: Duplicateauthentication method"
                                ));
                            }
                            None => {}
                        };
                        let length = u16::from_be_bytes(
                            value_slice[0..2]
                                .try_into()
                                .map_err(|_| anyhow::Error::msg("Invalid slice length for u8"))?,
                        ) as usize;
                        self.properties.authentication_method =
                            match std::str::from_utf8(&value_slice[2..2 + length]) {
                                Ok(v) => Some(v.to_string()),
                                Err(_) => {
                                    return Err(anyhow::anyhow!(
                                        "Protocol Error: authentication method property, not utf8"
                                    ))
                                }
                            };
                    }
                    0x16 => {
                        match self.properties.authentication_data {
                            Some(_) => {
                                return Err(anyhow::anyhow!(
                                    "Protocol Error: Duplicateauthentication data"
                                ));
                            }
                            None => {}
                        };
                        self.properties.authentication_data =
                            Some(bytes::Bytes::copy_from_slice(value_slice));
                    }

                    code => return Err(anyhow::anyhow!("Unknown Property {}", code)),
                }
            } else {
                return Err(anyhow::anyhow!("TLV Length Error"));
            }
            // next pos
            pos = pos + length;

            //3 4 5 6 7 | 8
            // end? //8  >= 3 + 4 -> 7
            // just ==
            if pos >= start_pos + property_length + 1 {
                break;
            }
        }
        return Ok(());
    }
    pub fn parse_payload(&mut self, buf: &bytes::Bytes) {}
}

impl Default for Connect {
    fn default() -> Self {
        Self {
            protocol_ver: ProtocolVersion::Other,
            clean_session: false,
            will: false,
            will_qos: 0,
            will_retain: false,
            user_password_flag: false,
            user_name_flag: false,
            client_id: "".to_string(),
            username: None,
            password: None,
            keepalive_timer: 0,
            properties: ConnectProperties::default(),
        }
    }
}

pub struct Disconnect {}

impl Default for Disconnect {
    fn default() -> Self {
        Self {}
    }
}

pub enum ProtocolVersion {
    V3,
    V3_1,
    V3_1_1,
    V5,
    Other,
}

mod mqtt {
    use super::{decode_variable_byte, Connect, ControlPacket, Disconnect, MqttPacket};
    use anyhow;

    pub fn parse_fixed_header(buf: &bytes::Bytes) -> Result<MqttPacket, anyhow::Error> {
        let packet: MqttPacket = match buf[0] >> 4 {
            0b0001 => {
                let (remaining_length, _) = decode_variable_byte(buf, 1);
                MqttPacket {
                    control_packet: ControlPacket::CONNECT(Connect::default()),
                    remaining_length: remaining_length,
                }
            }
            0b1110 => MqttPacket {
                control_packet: ControlPacket::DISCONNECT(Disconnect::default()),
                remaining_length: 0,
            },
            control_type => {
                return Err(anyhow::anyhow!(
                    "Control packet decode error {}",
                    control_type
                ))
            }
        };
        return Result::Ok(packet);
    }
}
