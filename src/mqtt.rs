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
    pub will_topic: String,
    pub will_payload: Option<bytes::Bytes>,
    pub user_password_flag: bool,
    pub user_name_flag: bool,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<bytes::Bytes>,
    pub keepalive_timer: u16,
    pub properties: ConnectProperties,
    pub will_properties: WillProperties,
}

pub struct ConnectProperties {
    pub session_expiry_interval: u32,
    pub receive_maximum: u16,
    pub maximum_packet_size: u32,
    pub topic_alias_maximum: u16,
    pub request_response_information: bool,
    pub request_problem_infromation: bool,
    pub user_properties: Vec<(String, String)>,
    pub authentication_method: Option<String>,
    pub authentication_data: Option<bytes::Bytes>,
}

pub struct WillProperties {
    pub will_delay_interval: u32,
    pub payload_format_indicator: Option<bool>,
    pub message_expiry_interval: Option<u32>,
    pub content_type: Option<String>,
    pub response_topic: Option<String>,
    pub correlation_data: Option<bytes::Bytes>,
    pub user_properties: Vec<(String, String)>,
    pub subscription_identifier: usize,
}

impl Default for WillProperties {
    fn default() -> Self {
        Self {
            will_delay_interval: 0,
            payload_format_indicator: None,
            message_expiry_interval: None,
            content_type: None,
            response_topic: None,
            correlation_data: None,
            user_properties: vec![],
            subscription_identifier: 0,
        }
    }
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

pub fn decode_u32_bytes(buf: &bytes::BytesMut, start_pos: usize) -> Result<u32, anyhow::Error> {
    Ok(u32::from_be_bytes(
        buf[start_pos..start_pos + 4]
            .try_into()
            .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
    ))
}

pub fn decode_u16_bytes(buf: &bytes::BytesMut, start_pos: usize) -> Result<u16, anyhow::Error> {
    Ok(u16::from_be_bytes(
        buf[start_pos..start_pos + 2]
            .try_into()
            .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
    ))
}

pub fn decode_u32_bytes_sliced(v: &[u8]) -> Result<u32, anyhow::Error> {
    Ok(u32::from_be_bytes(v.try_into().map_err(|_| {
        anyhow::Error::msg("Invalid slice length for u32")
    })?))
}

pub fn decode_utf8_string(
    buf: &bytes::BytesMut,
    start_pos: usize,
) -> Result<(String, usize), anyhow::Error> {
    let length = decode_u16_bytes(buf, start_pos)? as usize;
    let str = match std::str::from_utf8(&buf[start_pos + 2..start_pos + 2 + length]) {
        Ok(v) => v,
        Err(_) => return Err(anyhow::anyhow!("Protocol Error: Invalid utf8")),
    };
    Ok((str.to_string(), start_pos + 2 + length))
}

// return end position
pub fn decode_variable_byte(buf: &bytes::BytesMut, start_pos: usize) -> (usize, usize) {
    let mut remaining_length: usize = 0;
    let mut inc = 0;
    for pos in start_pos..=start_pos + 3 {
        remaining_length += ((buf[pos] & 0b01111111) as usize) << (inc * 7);
        if (buf[pos] & 0b10000000) == 0 {
            return (remaining_length, pos);
        }
        inc += 1;
    }
    (remaining_length, start_pos + 3)
}

impl Connect {
    // return consumed length
    pub fn parse_variable_header(&mut self, buf: &bytes::BytesMut) -> Result<usize, anyhow::Error> {
        let mut consumed_length = 0;
        let protocol_length = (((buf[0] as usize) << 8) + (buf[1] as usize)) as usize;
        println!("variable header: Variable length: {}", protocol_length);
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
                return Err(anyhow::anyhow!("Connect Protocol Header Error, length 4"));
            }
            self.parse_connect_flag(buf[7]);
            self.keepalive_timer = byte_pair_to_u16(buf[8], buf[9]);
            consumed_length = 10;
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
                return Err(anyhow::anyhow!("Connect Protocol Header Error, length 6"));
            }
            self.parse_connect_flag(buf[9]);
            self.keepalive_timer = byte_pair_to_u16(buf[10], buf[11]);
            consumed_length = 12;
        } else {
            return Err(anyhow::anyhow!(
                "Connect Protocol Header Error, length 6 not 4"
            ));
        }

        /* CONNECT Properties */
        match self.protocol_ver {
            ProtocolVersion::V5 => {
                let (property_length, end_pos) = decode_variable_byte(buf, 10);
                consumed_length = consumed_length + (end_pos - 10 + 1); /* length of length  10 11 12 13 -> 13 - 10 + 14 = 3*/
                // property length;
                /* variable  */
                self.parse_properties(buf, end_pos + 1, property_length)?;
                consumed_length = consumed_length + property_length;
            }
            _ => {}
        }

        Ok(consumed_length)
        // NEXT -> Connect Payload
    }

    fn parse_connect_flag(&mut self, b: u8) {
        self.clean_session = (b & 0b00000010) == 0b00000010;
        self.will = (b & 0b00000100) == 0b00000100;
        self.will_qos = (b & 0b00011000) >> 3;
        self.will_retain = (b & 0b00100000) == 0b00100000;
        self.user_password_flag = (b & 0b01000000) == 0b01000000;
        self.user_name_flag = (b & 0b10000000) == 0b01000000;
    }

    // return next position
    fn parse_properties(
        &mut self,
        buf: &bytes::BytesMut,
        start_pos: usize,
        property_length: usize,
    ) -> Result<(), anyhow::Error> {
        let mut pos = start_pos;
        loop {
            //3 4 5 6 7 | 8
            // start_pos = 3
            // propety_length = 5
            // pos = 8 -> OK
            if pos == start_pos + property_length {
                println!("OK!!!!!!");
                break;
            }
            // overrun
            if pos > start_pos + property_length {
                return Err(anyhow::anyhow!("Property Length Error"));
            }

            match buf[pos] {
                0x11 => {
                    self.properties.session_expiry_interval = u32::from_be_bytes(
                        buf[pos + 1..pos + 5]
                            .try_into()
                            .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
                    );
                    pos = pos + 5;
                }
                0x21 => {
                    self.properties.session_expiry_interval = u32::from_be_bytes(
                        buf[pos + 1..pos + 5]
                            .try_into()
                            .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
                    );
                    pos = pos + 5;
                }
                0x27 => {
                    self.properties.maximum_packet_size = u32::from_be_bytes(
                        buf[pos + 1..pos + 5]
                            .try_into()
                            .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
                    );
                    pos = pos + 5;
                }
                0x22 => {
                    self.properties.topic_alias_maximum = u16::from_be_bytes(
                        buf[pos + 1..pos + 3]
                            .try_into()
                            .map_err(|_| anyhow::Error::msg("Invalid slice length for u16"))?,
                    );
                    pos = pos + 3;
                }
                0x19 => {
                    self.properties.request_response_information = (buf[pos + 1] & 0b1) == 0b1;
                    pos = pos + 2;
                }
                0x17 => {
                    self.properties.request_problem_infromation = (buf[pos + 1] & 0b1) == 0b1;
                    pos = pos + 2;
                }
                0x26 => {
                    let (key, end_pos) = decode_utf8_string(&buf, 0)?;
                    let (value, end_pos) = decode_utf8_string(&buf, end_pos)?;
                    self.properties.user_properties.push((key, value));
                    pos = end_pos;
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

                    let (method, end_pos) = decode_utf8_string(buf, pos + 1)?;
                    self.properties.authentication_method = Some(method);
                    pos = end_pos;
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
                    let length = u16::from_be_bytes(
                        buf[pos + 1..pos + 3]
                            .try_into()
                            .map_err(|_| anyhow::Error::msg("Invalid slice length for u16"))?,
                    ) as usize;
                    let data = &buf[pos + 3..pos + 3 + length];
                    // copy
                    self.properties.authentication_data = Some(bytes::Bytes::copy_from_slice(data));
                    pos = pos + 3 + length;
                }

                code => return Err(anyhow::anyhow!("Unknown Property {}", code)),
            }
        }
        return Ok(());
    }
    pub fn parse_payload(&mut self, buf: &bytes::BytesMut) -> Result<(), anyhow::Error> {
        /* client id */
        // payload
        println!("{:#04X?}", &buf[0..2]);
        let mut pos = 0;
        {
            //decode_utf8_string
            let client_id_length = u16::from_be_bytes(
                buf[0..2]
                    .try_into()
                    .map_err(|_| anyhow::Error::msg("Invalid slice length for u8"))?,
            ) as usize;
            println!("lebnchh: {}", client_id_length);
            self.client_id = match std::str::from_utf8(&buf[2..2 + client_id_length]) {
                Ok(v) => v.to_string(),
                Err(_) => return Err(anyhow::anyhow!("Protocol: Client ID parse Error, not utf8")),
            };
            pos = 2 + client_id_length;
        }
        /* will property*/
        if self.will {
            let (will_property_length, ret_pos) = decode_variable_byte(buf, pos);
            pos = pos + ret_pos;
            loop {
                match buf[pos] {
                    0x18 => {
                        self.will_properties.will_delay_interval = decode_u32_bytes(buf, pos + 1)?;
                        pos = pos + 5;
                    }
                    0x01 => {
                        match self.will_properties.payload_format_indicator {
                            Some(_) => {
                                return Err(anyhow::anyhow!(
                                    "Protocol: Duplicate Payload format indicator"
                                ))
                            }
                            _ => {}
                        };
                        self.will_properties.payload_format_indicator = Some(buf[pos + 1] == 0b1);
                        pos = pos + 2;
                    }
                    0x02 => {
                        self.will_properties.message_expiry_interval =
                            Some(decode_u32_bytes(buf, pos + 1)?);
                        pos = pos + 5
                    }
                    0x03 => {
                        match self.will_properties.content_type {
                            Some(_) => {
                                return Err(anyhow::anyhow!("Protocol: Duplicate content type"));
                            }
                            _ => {}
                        };
                        let (content_type, end_pos) = decode_utf8_string(buf, pos + 1)?;
                        self.will_properties.content_type = Some(content_type);
                        pos = end_pos;
                    }
                    0x08 => {
                        //let end_pos;
                        let (response_topic, end_pos) = decode_utf8_string(buf, pos + 1)?;
                        self.will_properties.response_topic = Some(response_topic);

                        pos = end_pos;
                    }
                    0x09 => {
                        match self.will_properties.correlation_data {
                            Some(_) => {
                                return Err(anyhow::anyhow!(
                                    "Protocol: Duplicate will properties correlation data"
                                ));
                            }
                            _ => {}
                        };
                        let (datalength, end_pos) = decode_variable_byte(buf, pos + 1);
                        self.will_properties.correlation_data = Some(
                            bytes::Bytes::copy_from_slice(&buf[end_pos..end_pos + datalength]),
                        );

                        pos = end_pos + datalength;
                    }
                    0x26 => {
                        let (key, end_pos) = decode_utf8_string(buf, pos + 1)?;
                        let (value, end_pos) = decode_utf8_string(buf, end_pos)?;
                        self.will_properties.user_properties.push((key, value));
                        pos = end_pos;
                    }
                    0x0B => {
                        let (id, end_pos) = decode_variable_byte(buf, pos + 1);
                        self.will_properties.subscription_identifier = id;
                        pos = end_pos;
                    }

                    _ => {}
                }
                if will_property_length == pos + 1 {
                    break;
                } else if will_property_length < pos + 1 {
                    return Err(anyhow::anyhow!(
                        "Protocol: Protocol Length ( will properties)"
                    ));
                }
            }
        }
        /* will properties end */
        /* */
        if self.will {
            // will topic
            let (wt, end_pos) = decode_utf8_string(buf, pos)?;
            self.will_topic = wt;
            pos = end_pos;
            // will payload
            let datalength = decode_u16_bytes(buf, pos)? as usize;
            self.will_payload = Some(bytes::Bytes::copy_from_slice(
                &buf[pos + 2..pos + 2 + datalength],
            ));

            pos = pos + 2 + datalength;
        }
        if self.user_name_flag {
            let (str, end_pos) = decode_utf8_string(buf, pos)?;
            self.username = Some(str);
            pos = end_pos;
        }

        if self.user_password_flag {
            let datalength = decode_u16_bytes(buf, pos)? as usize;
            self.password = Some(bytes::Bytes::copy_from_slice(
                &buf[pos + 2..pos + 2 + datalength],
            ));
            pos = pos + 2 + datalength;
        }

        Ok(())
    }
}

impl Default for Connect {
    fn default() -> Self {
        Self {
            protocol_ver: ProtocolVersion::Other,
            clean_session: false,
            will: false,
            will_qos: 0,
            will_retain: false,
            will_payload: None,
            will_topic: "".to_string(),
            user_password_flag: false,
            user_name_flag: false,
            client_id: "".to_string(),
            username: None,
            password: None,
            keepalive_timer: 0,
            properties: ConnectProperties::default(),
            will_properties: WillProperties::default(),
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
    // (, consumed size)
    pub fn parse_fixed_header(buf: &bytes::BytesMut) -> Result<(MqttPacket, usize), anyhow::Error> {
        println!("{:#b}", buf[0]);
        let mut consumed_bytes = 0;
        let packet: MqttPacket = match buf[0] >> 4 {
            0b0001 => {
                let (remaining_length, endpos) = decode_variable_byte(buf, 1);
                println!("remain :{}", remaining_length);
                consumed_bytes = 1 /* header */ + (endpos - 1) + 1 /* end - start + 1 */;

                MqttPacket {
                    control_packet: ControlPacket::CONNECT(Connect::default()),
                    remaining_length: remaining_length,
                }
            }
            0b1110 => {
                let (remaining_length, endpos) = decode_variable_byte(buf, 1);
                consumed_bytes = 1 /* header */ + (endpos - 1) + 1;
                MqttPacket {
                    control_packet: ControlPacket::DISCONNECT(Disconnect::default()),
                    remaining_length: 0,
                }
            }
            control_type => {
                return Err(anyhow::anyhow!(
                    "Control packet decode error {}",
                    control_type
                ))
            }
        };
        return Result::Ok((packet, consumed_bytes));
    }
}

#[cfg(test)]
mod tests {
    use bytes::Buf;

    use super::mqtt::*;
    use crate::mqtt::ControlPacket;
    use crate::mqtt::*;
    fn decode_hex(str: &str) -> bytes::BytesMut {
        let decoded = hex::decode(str).unwrap();
        bytes::BytesMut::from(&decoded[..])
    }
    /*
    MQ Telemetry Transport Protocol, Connect Command
        Header Flags: 0x10, Message Type: Connect Command
            0001 .... = Message Type: Connect Command (1)
            .... 0000 = Reserved: 0
        Msg Len: 24
        Protocol Name Length: 4
        Protocol Name: MQTT
        Version: MQTT v5.0 (5)
        Connect Flags: 0x02, QoS Level: At most once delivery (Fire and Forget), Clean Session Flag
            0... .... = User Name Flag: Not set
            .0.. .... = Password Flag: Not set
            ..0. .... = Will Retain: Not set
            ...0 0... = QoS Level: At most once delivery (Fire and Forget) (0)
            .... .0.. = Will Flag: Not set
            .... ..1. = Clean Session Flag: Set
            .... ...0 = (Reserved): Not set
        Keep Alive: 60
        Properties
            Total Length: 0
        Client ID Length: 11
        Client ID: publish-549

         */
    #[test]
    fn connect_minimum() {
        let input = "101800044d5154540502003c00000b7075626c6973682d353439";
        let mut b = decode_hex(input);
        let ret = parse_fixed_header(&b);
        assert!(ret.is_ok());
        let (packet, consumed) = ret.unwrap();
        println!("aaaaaa:{}", consumed);
        b.advance(consumed);
        if let ControlPacket::CONNECT(mut connect) = packet.control_packet {
            // variable header
            let ret = connect.parse_variable_header(&b);
            assert!(ret.is_ok(), "Error: {}", ret.unwrap_err());
            match connect.protocol_ver {
                ProtocolVersion::V5 => {}
                _ => panic!("Protocol unmatch"),
            }
            assert_eq!(connect.clean_session, true);
            assert_eq!(connect.will, false);
            assert_eq!(connect.will_qos, 0);
            assert_eq!(connect.will_retain, false);
            assert_eq!(connect.user_name_flag, false);
            assert_eq!(connect.user_name_flag, false);
            assert_eq!(connect.keepalive_timer, 60);
            let consumed = ret.unwrap();
            println!("variable {}", consumed);
            b.advance(consumed);

            let res = connect.parse_payload(&b);
            assert!(res.is_ok());
            assert_eq!(connect.client_id, "publish-549".to_string());
        } else {
            panic!("packet type error")
        }
    }
}
