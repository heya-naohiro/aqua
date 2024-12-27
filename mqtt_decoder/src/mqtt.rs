use anyhow::Result;
use bytes::BufMut;
use std::u16;
use thiserror::Error;
use tower::retry::backoff::InvalidBackoff;

#[derive(Debug, Error)]
pub enum MqttError {
    #[error("Insufficient Bytes")]
    InsufficientBytes,
    #[error("Decode Error Invalid Parameter")]
    DecodeError,
}

pub enum ControlPacket {
    CONNECT(Connect),
    DISCONNECT(Disconnect),
    CONNACK(Connack),
    PUBLISH(Publish),
    /*
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

pub trait MqttPacket {
    fn parse_variable_header(&mut self, buf: &bytes::BytesMut) -> Result<usize, anyhow::Error>;
    fn parse_payload(&mut self, buf: &bytes::BytesMut) -> Result<usize, anyhow::Error>;
    fn remain_length(&self) -> usize;
}

impl MqttPacket for ControlPacket {
    fn parse_payload(&mut self, buf: &bytes::BytesMut) -> Result<usize, anyhow::Error> {
        match self {
            ControlPacket::CONNECT(p) => p.parse_payload(buf),
            ControlPacket::PUBLISH(p) => p.parse_payload(buf),
            _ => Err(anyhow::anyhow!("Not Implemented")),
        }
    }
    fn parse_variable_header(&mut self, buf: &bytes::BytesMut) -> Result<usize, anyhow::Error> {
        match self {
            ControlPacket::CONNECT(p) => p.parse_variable_header(buf),
            ControlPacket::PUBLISH(p) => p.parse_variable_header(buf),
            _ => Err(anyhow::anyhow!("Not Implemented")),
        }
    }
    fn remain_length(&self) -> usize {
        match self {
            ControlPacket::CONNECT(p) => p.remain_length,
            ControlPacket::PUBLISH(p) => p.remain_length,
            _ => 0,
        }
    }
}

#[derive(Default)]
pub struct Connack {
    pub remaining_length: usize,
    pub acknowledge_flag: bool,
    pub session_present: bool,
    pub connect_reason: ConnackReason,
    pub connack_properties: Option<ConnackProperties>,
}

#[derive(Default, Copy, Clone)]
pub enum ConnackReason {
    /* Success */
    #[default]
    Success = 0x00,
    /* Must Close */
    UnspecifiedError = 0x80,
    MalformedPacket = 0x81,
    ProtocolError = 0x82,
    ImplementationSpecificError = 0x83,
    UnsupportedProtocolVersion = 0x84,
    ClientIdentifierNotValid = 0x85,
    BadUserNameOrPassword = 0x86,
    NotAuthorized = 0x87,
    ServerUnavailable = 0x88,
    ServerBusy = 0x89,
    Banned = 0x8A,
    BadAuthenticationMethod = 0x8C,
    TopicNameInvalid = 0x90,
    PacketTooLarge = 0x95,
    QuotaExceeded = 0x97,
    PayloadFormatInvalid = 0x99,
    RetainNotSupported = 0x9A,
    QoSNotSupported = 0x9B,
    UseAnotherServer = 0x9C,
    ServerMoved = 0x9D,
    ConnectionRateExceeded = 0x9F,
}

#[derive(Default)]
pub struct ConnackProperties {
    pub session_expiry_interval: Option<u32>,
    pub receive_maximum: Option<u16>,
    pub maximum_qos: Option<u8>,
    pub retain_available: Option<bool>,
    pub maximum_packet_size: Option<u32>,
    pub assigned_client_identifier: Option<String>,
    pub topic_alias_maximum: Option<u16>,
    pub reason_string: Option<String>,
    pub user_properties: Vec<(String, String)>,
    pub wildcard_subscription_available: Option<u8>,
    pub subscription_identifiers_available: Option<u8>,
    pub shared_subscription_available: Option<u8>,
    pub server_keep_alive: Option<u16>,
    pub response_information: Option<String>,
    pub server_reference: Option<String>,
    pub authentication_method: Option<String>,
    pub authentication_data: Option<bytes::Bytes>,
}

impl ConnackProperties {
    fn build_bytes(&mut self) -> Result<bytes::Bytes> {
        let mut buf = bytes::BytesMut::new();

        if let Some(c) = self.session_expiry_interval {
            buf.extend_from_slice(&[0x11]);
            buf.extend_from_slice(&c.to_be_bytes());
        }
        if let Some(c) = self.receive_maximum {
            buf.extend_from_slice(&[0x21]);
            buf.extend_from_slice(&c.to_be_bytes());
        }
        if let Some(c) = self.maximum_qos {
            buf.extend_from_slice(&[0x24]);
            buf.extend_from_slice(&c.to_be_bytes());
        }
        if let Some(c) = self.retain_available {
            buf.extend_from_slice(&[0x25]);
            buf.extend_from_slice(bool_to_bytes(c));
        }
        if let Some(c) = self.maximum_packet_size {
            buf.extend_from_slice(&[0x27]);
            buf.extend_from_slice(&c.to_be_bytes());
        }
        if let Some(c) = &self.assigned_client_identifier {
            buf.extend_from_slice(&[0x12]);
            buf.extend_from_slice(c.as_bytes());
        }
        if let Some(c) = self.topic_alias_maximum {
            buf.extend_from_slice(&[0x22]);
            buf.extend_from_slice(&c.to_be_bytes());
        }
        if let Some(c) = &self.reason_string {
            buf.extend_from_slice(&[0x1f]);
            buf.extend_from_slice(c.as_bytes());
        }
        for v in &self.user_properties {
            buf.extend_from_slice(&[0x26]);
            let l: u16 = v.0.len().try_into()?;
            buf.extend_from_slice(&l.to_be_bytes());
            buf.extend_from_slice(v.0.as_bytes());
            let l: u16 = v.1.len().try_into()?;
            buf.extend_from_slice(&l.to_be_bytes());
            buf.extend_from_slice(v.1.as_bytes());
        }
        if let Some(c) = self.wildcard_subscription_available {
            buf.extend_from_slice(&[0x28]);
            buf.extend_from_slice(&c.to_be_bytes());
        }
        if let Some(c) = self.subscription_identifiers_available {
            buf.extend_from_slice(&[0x29]);
            buf.extend_from_slice(&c.to_be_bytes());
        }
        if let Some(c) = self.shared_subscription_available {
            buf.extend_from_slice(&[0x2a]);
            buf.extend_from_slice(&c.to_be_bytes());
        }
        if let Some(c) = self.server_keep_alive {
            buf.extend_from_slice(&[0x13]);
            buf.extend_from_slice(&c.to_be_bytes());
        }
        if let Some(c) = &self.response_information {
            buf.extend_from_slice(&[0x1a]);
            let l: u16 = c.len().try_into()?;
            buf.extend_from_slice(&l.to_be_bytes());
            buf.extend_from_slice(c.as_bytes());
        }
        if let Some(c) = &self.server_reference {
            buf.extend_from_slice(&[0x1c]);
            let l: u16 = c.len().try_into()?;
            buf.extend_from_slice(&l.to_be_bytes());
            buf.extend_from_slice(c.as_bytes());
        }
        if let Some(c) = &self.authentication_method {
            buf.extend_from_slice(&[0x15]);
            let l: u16 = c.len().try_into()?;
            buf.extend_from_slice(&l.to_be_bytes());
            buf.extend_from_slice(c.as_bytes());
        }
        if let Some(c) = &self.authentication_data {
            buf.extend_from_slice(&[0x16]);
            // ?
            buf.extend(c);
        }
        Ok(buf.freeze())
    }
}

impl Connack {
    fn build_bytes(&mut self) -> Result<bytes::Bytes> {
        let mut properties_len = 0;
        let mut properties_bytes = bytes::Bytes::new();
        if let Some(connack_properties) = &mut self.connack_properties {
            properties_bytes = connack_properties.build_bytes()?;
            properties_len = properties_bytes.len();
        }
        // remain length is properties only.
        let encoded_properties_len = encode_variable_bytes(properties_len);
        let mut buf =
            bytes::BytesMut::with_capacity(3 + encoded_properties_len.len() + properties_len);
        let remain_length =
            encode_variable_bytes(2 + encoded_properties_len.len() + properties_len);
        /* Fixed header */
        buf.put_u8(0b00100000);
        /* remaining length */
        buf.extend_from_slice(&remain_length);
        /* Variable header */
        /* 1byte */
        if self.session_present {
            buf.put_u8(0x01);
        } else {
            buf.put_u8(0x00);
        }
        /* 2byte */
        // Connect Reason Code
        buf.put_u8(self.connect_reason as u8);

        /* Properties length */
        buf.extend_from_slice(&encoded_properties_len);

        /* Properties */
        if let Some(_) = self.connack_properties {
            buf.extend_from_slice(&properties_bytes);
        }
        Ok(buf.freeze())
    }
}

#[derive(PartialEq, Debug)]
pub struct Publish {
    pub remain_length: usize,
    pub dup: bool,
    pub qos: QoS,
    pub retain: Retain,
    pub topic_name: Option<TopicName>,
    pub pub_properties: Vec<PublishProperties>,
    pub packet_id: PacketId, /* 2byte integer */
}

// [TODO] ConnectProperties / WillPropertyもこうする
#[derive(Debug, PartialEq)]
enum PublishProperties {
    PayloadFormatIndicator(PayloadFormatIndicator),
    MessageExpiryInterval(MessageExpiryInterval),
    TopicAlias(TopicAlias),
    ResponseTopic(ResponseTopic),
    CorrelationData(CorrelationData),
    UserProperty(UserProperty),
    SubscriptionIdentifier(SubscriptionIdentifier),
    ContentType(ContentType),
}

type PayloadFormatIndicator = bool;
type MessageExpiryInterval = u32;
type TopicAlias = u16;
type ResponseTopic = String;
type CorrelationData = bytes::Bytes;
type UserProperty = Vec<(String, String)>;
type SubscriptionIdentifier = u32;
type ContentType = String;

type Retain = bool;
type PacketId = u16;
type TopicName = String;

#[derive(PartialEq, Debug)]
enum QoS {
    QoS0,
    QoS1,
    QoS2,
}

impl TryFrom<u8> for QoS {
    type Error = MqttError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(QoS::QoS0),
            1 => Ok(QoS::QoS1),
            2 => Ok(QoS::QoS2),
            _ => Err(MqttError::DecodeError),
        }
    }
}

#[derive(PartialEq, Debug)]
pub struct Connect {
    pub remain_length: usize,
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
#[derive(PartialEq, Debug)]
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
#[derive(PartialEq, Debug)]
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

#[inline]
fn bool_to_bytes(b: bool) -> &'static [u8] {
    if b {
        &[1u8] // trueは1に変換
    } else {
        &[0u8] // falseは0に変換
    }
}

pub fn decode_u32_bytes(buf: &bytes::BytesMut, start_pos: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        buf[start_pos..start_pos + 4]
            .try_into()
            .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
    ))
}

pub fn decode_u16_bytes(buf: &bytes::BytesMut, start_pos: usize) -> Result<u16> {
    Ok(u16::from_be_bytes(
        buf[start_pos..start_pos + 2]
            .try_into()
            .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
    ))
}

pub fn decode_u32_bytes_sliced(v: &[u8]) -> Result<u32> {
    Ok(u32::from_be_bytes(v.try_into().map_err(|_| {
        anyhow::Error::msg("Invalid slice length for u32")
    })?))
}

pub fn decode_utf8_string(
    buf: &bytes::BytesMut,
    start_pos: usize,
) -> Result<(String, usize), anyhow::Error> {
    let length = decode_u16_bytes(buf, start_pos)? as usize;
    println!("length: {}", length);

    if start_pos + 2 + length > buf.len() {
        return Err(MqttError::InsufficientBytes.into());
    }

    let str = match std::str::from_utf8(&buf[start_pos + 2..start_pos + 2 + length]) {
        Ok(v) => v,
        Err(_) => return Err(anyhow::anyhow!("Protocol Error: Invalid utf8")),
    };
    Ok((str.to_string(), start_pos + 2 + length))
}

// error
// return end position(end is consumed already)
pub fn decode_variable_length(
    buf: &bytes::BytesMut,
    start_pos: usize,
) -> Result<(usize, usize), MqttError> {
    let mut remaining_length: usize = 0;
    let mut inc = 0;
    for pos in start_pos..=start_pos + 3 {
        if buf.len() < pos {
            return Err(MqttError::InsufficientBytes);
        }
        remaining_length += ((buf[pos] & 0b01111111) as usize) << (inc * 7);
        if (buf[pos] & 0b10000000) == 0 {
            return Ok((remaining_length, pos));
        }
        inc += 1;
    }
    Ok((remaining_length, start_pos + 3))
}

fn encode_variable_bytes(mut length: usize) -> Vec<u8> {
    let mut remaining_length = Vec::new();
    for _ in 1..=4 {
        let mut digit = (length % 128) as u8;
        length /= 128;
        if length > 0 {
            digit |= 0x80;
        }
        remaining_length.push(digit);
        if length == 0 {
            break;
        }
    }
    remaining_length
}

impl MqttPacket for Publish {
    fn parse_variable_header(&mut self, buf: &bytes::BytesMut) -> Result<usize, anyhow::Error> {
        let mut consumed_length = 0;
        // topic name UTF-8 Encoded String
        let (topic_name, next_pos) = decode_utf8_string(buf, 0)?;
        self.topic_name = Some(topic_name);
        consumed_length = next_pos;

        // packet identifier
        if self.qos == QoS::QoS1 || self.qos == QoS::QoS2 {
            // check
            if buf.len() < next_pos + 1 {
                return Err(MqttError::InsufficientBytes.into());
            }
            self.packet_id = ((buf[next_pos] as u16) << 8) + buf[next_pos + 1] as u16;
            consumed_length = consumed_length + 2;
        }
        Ok(consumed_length)
    }

    fn parse_payload(&mut self, buf: &bytes::BytesMut) -> Result<usize, anyhow::Error> {
        todo!();
    }

    fn remain_length(&self) -> usize {
        todo!();
    }
}

impl MqttPacket for Connect {
    // return consumed length
    fn parse_variable_header(&mut self, buf: &bytes::BytesMut) -> Result<usize, anyhow::Error> {
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
                let (property_length, end_pos) = decode_variable_length(buf, 10)?;
                consumed_length = consumed_length + (end_pos - 10 + 1); /* length of length  10 11 12 13 -> 13 - 10 + 14 = 3 */
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

    fn parse_payload(&mut self, buf: &bytes::BytesMut) -> Result<usize, anyhow::Error> {
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
            self.client_id = match std::str::from_utf8(&buf[2..2 + client_id_length]) {
                Ok(v) => v.to_string(),
                Err(_) => return Err(anyhow::anyhow!("Protocol: Client ID parse Error, not utf8")),
            };
            pos = 2 + client_id_length;
        }
        /* will property*/
        /* */
        if self.will {
            let (will_property_length, end_pos) = decode_variable_length(buf, pos)?;
            println!("will property length {}", will_property_length);
            pos = end_pos + 1;
            let first_will_property_position = pos;
            loop {
                // 3 4 5 6 7 8
                // 8 - 3 = 5
                if pos - first_will_property_position == will_property_length {
                    break;
                } else if pos - first_will_property_position > will_property_length {
                    return Err(anyhow::anyhow!(
                        "Protocol: Protocol Length overrun (will properties)"
                    ));
                }
                println!("will property 0x{:x}", buf[pos]);
                match buf[pos] {
                    0x18 => {
                        self.will_properties.will_delay_interval = decode_u32_bytes(buf, pos + 1)?;
                        println!(
                            "will deray interval {}",
                            self.will_properties.will_delay_interval
                        );
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
                        //?? Binary Data is represented by a Two Byte Integer length which indicates the number of data bytes, followed by that number of bytes. Thus, the length of Binary Data is limited to the range of 0 to 65,535 Bytes.
                        match self.will_properties.correlation_data {
                            Some(_) => {
                                return Err(anyhow::anyhow!(
                                    "Protocol: Duplicate will properties correlation data"
                                ));
                            }
                            _ => {}
                        };
                        let datalength = decode_u16_bytes(buf, pos + 1)? as usize;
                        println!("0x09 correlation_data length {:?}", datalength);
                        self.will_properties.correlation_data = Some(
                            bytes::Bytes::copy_from_slice(&buf[pos + 3..pos + 3 + datalength]),
                        );
                        println!(
                            "0x09 correlation_data {:?}",
                            self.will_properties.correlation_data
                        );
                        pos = pos + 3 + datalength;
                    }
                    0x26 => {
                        let (key, end_pos) = decode_utf8_string(buf, pos + 1)?;
                        let (value, end_pos) = decode_utf8_string(buf, end_pos)?;
                        self.will_properties.user_properties.push((key, value));
                        pos = end_pos;
                    }
                    0x0B => {
                        let (id, end_pos) = decode_variable_length(buf, pos + 1)?;
                        self.will_properties.subscription_identifier = id;
                        pos = end_pos + 1;
                    }

                    code => {
                        return Err(anyhow::anyhow!(
                            "Protocol: Unknown Will Property Error 0x{:x}",
                            code
                        ));
                    }
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

        Ok(pos)
    }

    fn remain_length(&self) -> usize {
        self.remain_length
    }
}
impl Connect {
    fn new(remain_length: usize) -> Self {
        Self {
            remain_length: remain_length,
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

    fn parse_connect_flag(&mut self, b: u8) {
        self.clean_session = (b & 0b00000010) == 0b00000010;
        self.will = (b & 0b00000100) == 0b00000100;
        self.will_qos = (b & 0b00011000) >> 3;
        self.will_retain = (b & 0b00100000) == 0b00100000;
        self.user_password_flag = (b & 0b01000000) == 0b01000000;
        self.user_name_flag = (b & 0b10000000) == 0b10000000;
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
            // pos means nextpos here.
            if pos == start_pos + property_length {
                break;
            }
            // overrun
            if pos > start_pos + property_length {
                return Err(anyhow::anyhow!("Property Length Error"));
            }
            println!("Connect Property 0x{:x}", buf[pos]);
            match buf[pos] {
                0x11 => {
                    self.properties.session_expiry_interval = u32::from_be_bytes(
                        buf[pos + 1..pos + 5]
                            .try_into()
                            .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
                    );
                    pos = pos + 5;
                    println!("0x11, {}", self.properties.session_expiry_interval)
                }
                0x21 => {
                    self.properties.receive_maximum = u16::from_be_bytes(
                        buf[pos + 1..pos + 3]
                            .try_into()
                            .map_err(|_| anyhow::Error::msg("Invalid slice length for u32"))?,
                    );
                    pos = pos + 3;
                    println!("0x21, {}", self.properties.receive_maximum);
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
                    println!("0x22, {}", self.properties.topic_alias_maximum);
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
                    println!("User property, debug");
                    let (key, end_pos) = decode_utf8_string(&buf, pos + 1)?;
                    println!("key: {}", key);
                    let (value, end_pos) = decode_utf8_string(&buf, end_pos)?;
                    println!("value: {}", key);
                    self.properties.user_properties.push((key, value));
                    pos = end_pos;
                    println!("0x26, {:#?}", self.properties.user_properties)
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

                code => return Err(anyhow::anyhow!("Unknown Connect Property 0x{:x}", code)),
            }
        }
        return Ok(());
    }
}

pub struct Disconnect {
    remain_length: usize,
}

impl Disconnect {
    fn new(remain_length: usize) -> Self {
        Self {
            remain_length: remain_length,
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum ProtocolVersion {
    V3,
    V3_1,
    V3_1_1,
    V5,
    Other,
}

pub mod parser {
    use super::{decode_variable_length, Connect, ControlPacket, Disconnect, MqttPacket};
    use anyhow;
    // (, consumed size)
    pub fn parse_fixed_header(
        buf: &bytes::BytesMut,
    ) -> Result<(ControlPacket, usize), anyhow::Error> {
        println!("{:#b}", buf[0]);
        match buf[0] >> 4 {
            0b0001 => {
                let (remaining_length, endpos) = decode_variable_length(buf, 1)?;
                println!("remain :{}", remaining_length);
                let consumed_bytes = 1 /* header */ + (endpos - 1) + 1 /* end - start + 1 */;

                return Result::Ok((
                    ControlPacket::CONNECT(Connect::new(remaining_length)),
                    consumed_bytes,
                ));
            }
            0b1110 => {
                let (remaining_length, endpos) = decode_variable_length(buf, 1)?;
                let consumed_bytes = 1 /* header */ + (endpos - 1) + 1;
                return Result::Ok((
                    ControlPacket::DISCONNECT(Disconnect::new(remaining_length)),
                    consumed_bytes,
                ));
            }
            control_type => {
                return Err(anyhow::anyhow!(
                    "Control packet decode error {}",
                    control_type
                ))
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use bytes::Buf;

    use super::parser::*;
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
    /* v5 */
    #[test]
    fn connect_minimum_parse() {
        let input = "101800044d5154540502003c00000b7075626c6973682d353439";
        let mut b = decode_hex(input);
        let ret = parse_fixed_header(&b);
        assert!(ret.is_ok());
        let (packet, consumed) = ret.unwrap();
        println!("aaaaaa:{}", consumed);
        b.advance(consumed);
        if let ControlPacket::CONNECT(mut connect) = packet {
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
            assert_eq!(connect.user_password_flag, false);
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

    #[test]
    fn mqtt5_connect_all_parse() {
        let input = "10bd0200044d51545405ce003c7c110000007\
        81500136578616d706c655f617574685f6d6574686f6416001\
        16578616d706c655f617574685f64617461170119012100142\
        2000a26000c636f6e6e6563745f6b657931000e636f6e6e656\
        3745f76616c75653126000c636f6e6e6563745f6b657932000\
        e636f6e6e6563745f76616c7565322700040000000b7075626\
        c6973682d373130710101020000001e03000a746578742f706\
        c61696e08000d746573742f726573706f6e7365090018636f7\
        272656c6174696f6e5f646174615f6578616d706c651800000\
        00a2600046b657931000676616c7565312600046b657932000\
        676616c7565322600046b657933000676616c7565330009746\
        573742f77696c6c000c57696c6c206d657373616765000d796\
        f75725f757365726e616d65000d796f75725f70617373776f7264";

        let mut b = decode_hex(input);
        let ret = parse_fixed_header(&b);
        assert!(ret.is_ok());
        let (packet, consumed) = ret.unwrap();
        println!("aaaaaa:{}", consumed);
        b.advance(consumed);
        if let ControlPacket::CONNECT(mut connect) = packet {
            let ret = connect.parse_variable_header(&b);
            println!("Hello");
            assert!(ret.is_ok(), "Error: {}", ret.unwrap_err());
            match connect.protocol_ver {
                ProtocolVersion::V5 => {}
                _ => panic!("Protocol unmatch"),
            }
            let consumed = ret.unwrap();
            println!("variable {}", consumed);
            b.advance(consumed);

            let res = connect.parse_payload(&b);
            assert!(res.is_ok());
            assert_eq!(connect.client_id, "publish-710".to_string());
            println!(">>>> {:?}", connect);

            assert_eq!(connect.clean_session, true);
            assert_eq!(connect.will, true);
            assert_eq!(connect.will_qos, 1);
            assert_eq!(connect.will_retain, false);
            assert_eq!(connect.user_name_flag, true);
            assert_eq!(connect.user_password_flag, true);
            assert_eq!(connect.keepalive_timer, 60);

            assert_eq!(connect.properties.session_expiry_interval, 120);
            assert_eq!(connect.properties.receive_maximum, 20);
            assert_eq!(connect.properties.maximum_packet_size, 256 * 1024);
            assert_eq!(connect.properties.topic_alias_maximum, 10);
            assert_eq!(connect.properties.request_problem_infromation, true);
            assert_eq!(connect.properties.request_response_information, true);
            assert_eq!(
                connect.properties.user_properties,
                [
                    ("connect_key1".to_string(), "connect_value1".to_string()),
                    ("connect_key2".to_string(), "connect_value2".to_string())
                ]
            );
            assert_eq!(
                connect.properties.authentication_method,
                Some("example_auth_method".to_string())
            );
            assert_eq!(
                connect.properties.authentication_data,
                Some(bytes::Bytes::copy_from_slice(
                    "example_auth_data".as_bytes()
                ))
            );
            /* will */
            assert_eq!(connect.will_properties.will_delay_interval, 10);
            assert_eq!(connect.will_properties.message_expiry_interval, Some(30));
            assert_eq!(
                connect.will_properties.content_type,
                Some("text/plain".to_string())
            );
            assert_eq!(connect.will_properties.payload_format_indicator, Some(true));
            assert_eq!(
                connect.will_properties.response_topic,
                Some("test/response".to_string())
            );
            assert_eq!(
                connect.will_properties.correlation_data,
                Some(bytes::Bytes::copy_from_slice(
                    "correlation_data_example".as_bytes()
                ))
            );
            assert_eq!(
                connect.will_properties.user_properties,
                [
                    ("key1".to_string(), "value1".to_string()),
                    ("key2".to_string(), "value2".to_string()),
                    ("key3".to_string(), "value3".to_string())
                ]
            );
        }
    }

    #[test]
    fn write_connack_success() {
        /*
        20 03              // 固定ヘッダー: パケットタイプ(0x20) + 残りの長さ(3バイト)
        00                 // 可変ヘッダー: 接続確認フラグ（セッションプレゼントなし）
        00                 // 可変ヘッダー: 接続応答コード (0x00 = 接続成功)
        00                 // プロパティの長さ (プロパティなし)
        */
        let expected: &[u8] = &[0x20, 0x03, 0x00, 0x00, 0x00];
        let mut connack = Connack {
            remaining_length: 0,
            acknowledge_flag: false,
            session_present: false,
            connect_reason: ConnackReason::Success,
            connack_properties: None,
        };
        let result_bytes = connack.build_bytes().unwrap();
        assert_eq!(result_bytes.as_ref(), expected);
    }

    #[test]
    fn write_connack_reject() {
        /*
        20 03              // 固定ヘッダー: パケットタイプ(0x20) + 残りの長さ(3バイト)
        00                 // 可変ヘッダー: 接続確認フラグ（セッションプレゼントなし）
        82                 // 可変ヘッダー: 接続応答コード (0x87 = 認証エラー)
        00                 // プロパティの長さ (プロパティなし)
        */
        let expected: &[u8] = &[0x20, 0x03, 0x00, 0x87, 0x00];
        let mut connack = Connack {
            remaining_length: 0,
            acknowledge_flag: false,
            session_present: false,
            connect_reason: ConnackReason::NotAuthorized,
            connack_properties: None,
        };
        let result_bytes = connack.build_bytes().unwrap();
        assert_eq!(result_bytes.as_ref(), expected);
    }

    #[test]
    fn write_connack_success_with_properties() {
        /*
        20 0D              // 固定ヘッダー: パケットタイプ(0x20) + 残りの長さ(13バイト)
        00                 // 可変ヘッダー: 接続確認フラグ（セッションプレゼントなし）
        00                 // 可変ヘッダー: 接続応答コード (0x00 = 接続成功)
        0A                 // プロパティ長 (10バイト)
        11                 // セッション有効期間 (Property Identifier = 0x11)
        00 00 0E 10        // セッション有効期間の値 (3600秒 = 1時間)
        27                 // 受信最大 (Property Identifier = 0x27)
        00 00 00 0A              // 受信最大の値 (10メッセージ)
        */
        let expected: &[u8] = &[
            0x20, 0x0D, 0x00, 0x00, 0x0A, 0x11, 0x00, 0x00, 0x0E, 0x10, 0x27, 0x00, 0x00, 0x00,
            0x0A,
        ];
        let mut p = ConnackProperties::default();
        p.session_expiry_interval = Some(3600);
        p.maximum_packet_size = Some(10);
        let mut connack = Connack {
            remaining_length: 0,
            acknowledge_flag: false,
            session_present: false,
            connect_reason: ConnackReason::Success,
            connack_properties: Some(p),
        };

        let result_bytes = connack.build_bytes().unwrap();
        assert_eq!(result_bytes.as_ref(), expected);
    }
}
