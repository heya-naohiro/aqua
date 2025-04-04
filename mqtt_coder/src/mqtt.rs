use std::fmt;

use bytes::{BufMut, Bytes, BytesMut};
use hex::encode;
use thiserror::Error;

use crate::mqtt;
#[derive(Debug, Error)]
pub enum MqttError {
    #[error("Insufficient Bytes")]
    InsufficientBytes,
    #[error("Decode Error Invalid Parameter")]
    InvalidFormat,
    #[error("Not Implemented")]
    NotImplemented,
    #[error("Unexpected")]
    Unexpected,
}

#[derive(Default, Debug)]
pub enum ControlPacket {
    CONNECT(Connect),
    DISCONNECT(Disconnect),
    CONNACK(Connack),
    PUBLISH(Publish),
    #[default]
    UNDEFINED,
    SUBSCRIBE(Subscribe),
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
    fn decode_variable_header(
        &mut self,
        buf: &bytes::BytesMut,
        start_pos: usize,
        protocol_version: Option<ProtocolVersion>,
    ) -> Result<usize, MqttError>;
    fn decode_payload(
        &mut self,
        buf: &bytes::BytesMut,
        start_pos: usize,
        protocol_version: Option<ProtocolVersion>,
    ) -> Result<usize, MqttError>;
    fn encode_header(&self) -> Result<Bytes, MqttError>;
    fn encode_payload_chunk(&self) -> Result<Option<Bytes>, MqttError>;
}

impl MqttPacket for ControlPacket {
    fn decode_payload(
        &mut self,
        buf: &bytes::BytesMut,
        start_pos: usize,
        protocol_version: Option<ProtocolVersion>
    ) -> Result<usize, MqttError> {
        match self {
            ControlPacket::CONNECT(p) => p.decode_payload(buf, start_pos, protocol_version),
            ControlPacket::PUBLISH(p) => p.decode_payload(buf, start_pos, protocol_version),
            ControlPacket::SUBSCRIBE(p) => p.decode_payload(buf, start_pos, protocol_version),
            _ => Err(MqttError::NotImplemented),
        }
    }
    fn decode_variable_header(
        &mut self,
        buf: &bytes::BytesMut,
        start_pos: usize,
        protocol_version: Option<ProtocolVersion>
    ) -> Result<usize, MqttError> {
        match self {
            ControlPacket::CONNECT(p) => p.decode_variable_header(buf, start_pos, protocol_version),
            ControlPacket::PUBLISH(p) => p.decode_variable_header(buf, start_pos, protocol_version),
            ControlPacket::SUBSCRIBE(p) => p.decode_variable_header(buf, start_pos, protocol_version),
            _ => Err(MqttError::NotImplemented),
        }
    }
    fn encode_header(&self) -> Result<Bytes, MqttError> {
        match self {
            ControlPacket::CONNACK(p) => p.encode_header(),
            _ => Err(MqttError::NotImplemented),
        }
    }

    fn encode_payload_chunk(&self) -> Result<Option<Bytes>, MqttError> {
        match self {
            ControlPacket::CONNACK(p) => p.encode_payload_chunk(),
            _ => Err(MqttError::NotImplemented),
        }
    }
}

#[derive(Default, Debug)]
pub struct Connack {
    pub remaining_length: usize,
    pub session_present: bool,
    pub connect_reason: ConnackReason,
    pub connack_properties: Option<ConnackProperties>,
    pub version: ProtocolVersion,
}

#[derive(Default, Copy, Clone, Debug, PartialEq)]
#[repr(u8)]
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
    KeepAliveTimeout = 0x8D,
    SessionTakenOver = 0x8E,
    TopicFilterInvalid = 0x8F,
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

impl ConnackReason {
    /// 数値から `ConnackReasonCode` へ変換
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x00 => Some(Self::Success),
            0x80 => Some(Self::UnspecifiedError),
            0x81 => Some(Self::MalformedPacket),
            0x82 => Some(Self::ProtocolError),
            0x83 => Some(Self::ImplementationSpecificError),
            0x84 => Some(Self::UnsupportedProtocolVersion),
            0x85 => Some(Self::ClientIdentifierNotValid),
            0x86 => Some(Self::BadUserNameOrPassword),
            0x87 => Some(Self::NotAuthorized),
            0x88 => Some(Self::ServerUnavailable),
            0x89 => Some(Self::ServerBusy),
            0x8A => Some(Self::Banned),
            0x8C => Some(Self::BadAuthenticationMethod),
            0x8D => Some(Self::KeepAliveTimeout),
            0x8E => Some(Self::SessionTakenOver),
            0x8F => Some(Self::TopicFilterInvalid),
            0x90 => Some(Self::TopicNameInvalid),
            0x95 => Some(Self::PacketTooLarge),
            0x97 => Some(Self::QuotaExceeded),
            0x99 => Some(Self::PayloadFormatInvalid),
            0x9A => Some(Self::RetainNotSupported),
            0x9B => Some(Self::QoSNotSupported),
            0x9C => Some(Self::UseAnotherServer),
            0x9D => Some(Self::ServerMoved),
            0x9F => Some(Self::ConnectionRateExceeded),
            _ => None,
        }
    }
}

#[derive(Default, Debug)]
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
    fn build_bytes(&self) -> std::result::Result<bytes::Bytes, MqttError> {
        let mut buf = BytesMut::new();
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
            let l: u16 = v.0.len().try_into().map_err(|_| MqttError::InvalidFormat)?;
            buf.extend_from_slice(&l.to_be_bytes());
            buf.extend_from_slice(v.0.as_bytes());
            let l: u16 = v.1.len().try_into().map_err(|_| MqttError::InvalidFormat)?;
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
            let l: u16 = c.len().try_into().map_err(|_| MqttError::InvalidFormat)?;
            buf.extend_from_slice(&l.to_be_bytes());
            buf.extend_from_slice(c.as_bytes());
        }
        if let Some(c) = &self.server_reference {
            buf.extend_from_slice(&[0x1c]);
            let l: u16 = c.len().try_into().map_err(|_| MqttError::InvalidFormat)?;
            buf.extend_from_slice(&l.to_be_bytes());
            buf.extend_from_slice(c.as_bytes());
        }
        if let Some(c) = &self.authentication_method {
            buf.extend_from_slice(&[0x15]);
            let l: u16 = c.len().try_into().map_err(|_| MqttError::InvalidFormat)?;
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
    fn encode_header(&self) -> Result<Bytes, MqttError> {
        self.build_bytes()
    }
    fn encode_payload_chunk(&self) -> Result<Option<Bytes>, MqttError> {
        Ok(None)
    }
    pub fn build_bytes(&self) -> std::result::Result<bytes::Bytes, MqttError> {
        dbg!(&self);
        let mut buf;
        let mut encoded_properties_len: Option<Vec<u8>> = None;
        let mut properties_bytes: Option<bytes::Bytes> = None;
        let mut prop_len = 0;
        if self.version == ProtocolVersion(0x05) {
            if let Some(connack_properties) = &self.connack_properties {
                let prop_bytes = connack_properties.build_bytes()?;
                prop_len = prop_bytes.len();
                let encoded_len = encode_variable_bytes(prop_len);
                encoded_properties_len = Some(encoded_len);
                properties_bytes = Some(prop_bytes);
            } else {
                encoded_properties_len = Some(encode_variable_bytes(0));
                properties_bytes = Some(bytes::Bytes::new());
            }
            // remain length is properties only.
        }

        // with capacityは自動で拡張されるので問題ない
        // 3 = header / remaining length / propertyの長さ
        let remaining_length = 2 /* session present flag and Connection return code */ + 
        if self.version == mqtt::ProtocolVersion(0x05) { 1 /* props length */ + prop_len } else { 0 };
        
        buf = bytes::BytesMut::with_capacity(
            5 /* 1bytes contrl header + variable max: 4 bytes*/ + remaining_length
        );
        //
        let encoded_remaining_length = encode_variable_bytes(
            remaining_length
        );
        /* Fixed header */
        buf.put_u8(0b00100000);
        /* remaining length */
        buf.extend_from_slice(&encoded_remaining_length);
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

        if self.version == ProtocolVersion(0x05) {
            /* Properties length */
            if let Some(props_length) = encoded_properties_len {
                buf.extend_from_slice(&props_length);
            }

            /* Properties */
            if let Some(props_bytes) = properties_bytes {
                buf.extend_from_slice(&props_bytes);
            }
        }
        dbg!(format!("0x{}", encode(&buf)));
        Ok(buf.freeze())
    }
}

#[derive(PartialEq, Debug, Default)]
pub struct Subscribe {
    pub remain_length: usize,
    pub packet_id: PacketId,
    pub sub_properties: SubscribeProperties,
    /* payload */
    pub topic_filters: Vec<(TopicFilter, Option<SubscribeOption>)>,
    payload_length: usize,
}

#[derive(PartialEq, Debug, Default)]
pub struct SubscribeProperties {
    subscription_identifier: Option<SubscriptionIdentifier>,
    user_properties: Option<Vec<UserProperty>>,
}

#[derive(PartialEq, Debug, Default)]
pub struct TopicFilter(String);
impl TopicFilter {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let (topic_name, next_pos) = decode_utf8_string(buf, start_pos)?;
        return Ok((TopicFilter(topic_name), next_pos));
    }
}

#[derive(PartialEq, Debug, Default)]
pub struct SubscribeOption {
    pub retain_handling: RetainHandling,
    pub retain_as_published: RetainAsPublished,
    pub no_local: NoLocal,
    pub qos: QoS,
}
impl SubscribeOption {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let b = buf[start_pos];
        let qos = QoS::try_from(b & 0b00000011)?;
        let nl = NoLocal((b & 0b00000100) != 0);
        let rap = RetainAsPublished((b & 0b00001000) != 0);
        let rh = RetainHandling::try_from(b >> 4)?;
        return Ok((SubscribeOption{
            retain_handling: rh,
            retain_as_published: rap,
            no_local: nl,
            qos: qos
        }, start_pos + 1))
    }
}

#[derive(PartialEq, Debug, Default)]
enum RetainHandling {
    #[default]
    ReceiveAll,
    IgnoreRetainedInitial,
    IgnoreRetainedAlways,
}
impl TryFrom<u8> for RetainHandling {
    type Error = MqttError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(RetainHandling::ReceiveAll),
            1 => Ok(RetainHandling::IgnoreRetainedInitial),
            2 => Ok(RetainHandling::IgnoreRetainedAlways),
            _ => Err(MqttError::InvalidFormat),
        }
    }
}

#[derive(PartialEq, Debug, Default)]
struct RetainAsPublished(bool);

#[derive(PartialEq, Debug, Default)]
struct NoLocal(bool);

#[derive(PartialEq, Debug, Default)]
pub struct Publish {
    pub remain_length: usize,
    pub dup: Dup,
    pub qos: QoS,
    pub retain: Retain,
    pub topic_name: Option<TopicName>,
    pub pub_properties: PublishProperties,
    pub packet_id: Option<PacketId>, /* 2byte integer */
}

#[derive(Debug, PartialEq, Clone, Default)]
struct Retain(bool); // fixed header

#[derive(Debug, PartialEq, Clone, Default)]
struct Dup(bool); // fixed header

fn decode_lower_fixed_header(
    buf: &bytes::BytesMut,
    start_pos: usize,
) -> Result<(Dup, QoS, Retain), MqttError> {
    let q = match (buf[start_pos] & 0b0000110) >> 1 {
        0b00 => QoS::QoS0,
        0b01 => QoS::QoS1,
        0b11 => QoS::QoS2,
        _ => return Err(MqttError::InvalidFormat),
    };

    Ok((
        Dup((buf[start_pos] & 0b00001000) != 0),
        q,
        Retain((buf[start_pos] & 0b00000001) != 0),
    ))
}

#[derive(Debug, PartialEq, Clone)]
struct TopicName(String); // variable header

impl TopicName {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let (topic_name, next_pos) = decode_utf8_string(buf, start_pos)?;
        Ok((TopicName(topic_name), next_pos))
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
struct PacketId(u16);
impl PacketId {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        Ok((
            PacketId(((buf[start_pos] as u16) << 8) + buf[start_pos + 1] as u16),
            start_pos + 2,
        ))
    }
}

// Property keys
const PROPERTY_PAYLOAD_FORMAT_INDICATOR_ID: u8 = 0x01;
const PROPERTY_MESSAGE_EXPIRY_INTERVAL_ID: u8 = 0x02;
const PROPERTY_TOPIC_ALIAS_ID: u8 = 0x23;
const PROPERTY_RESPONSE_TOPIC_ID: u8 = 0x08;
const PROPERTY_CORRELATION_DATA_ID: u8 = 0x09;
const PROPERTY_USER_PROPERTY_ID: u8 = 0x26;
const PROPERTY_SUBSCRIPTION_IDENTIFIER_ID: u8 = 0x0b;
const PROPERTY_CONTENT_TYPE_ID: u8 = 0x03;

// Property keys
const PROPERTY_SESSION_EXPIRY_INTERVAL: u8 = 0x11;
const PROPERTY_RECIEVE_MAXIMUM: u8 = 0x21;
const PROPERTY_MAXIMUM_PACKET_SIZE: u8 = 0x27;
const PROPERTY_TOPIC_ALIAS_MAXIMUM: u8 = 0x22;
const PROPERTY_REQUEST_RESPONSE_INFORMATION: u8 = 0x19;
const PROPERTY_REQUEST_PROBLEM_INFORMATION: u8 = 0x17;
const PROPERTY_AUTHENTICATION_METHOD: u8 = 0x15;
const PROPERTY_AUTHENTICATION_DATA: u8 = 0x16;

// Property keys
const PROPERTY_WILL_DELAY_INTERVAL: u8 = 0x18;

// [TODO] ConnectProperties / WillPropertyもこうする
#[derive(PartialEq, Debug, Default)]
struct PublishProperties {
    payload_format_indicator: Option<PayloadFormatIndicator>,
    message_expiry_interval: Option<MessageExpiryInterval>,
    topic_alias: Option<TopicAlias>,
    response_topic: Option<ResponseTopic>,
    correlation_data: Option<CorrelationData>,
    user_properties: Option<Vec<UserProperty>>,
    subscription_identifier: Option<Vec<SubscriptionIdentifier>>, // Multiple Subscription Identifiers will be included
    content_type: Option<ContentType>,
}

#[derive(Debug, PartialEq, Clone)]
#[repr(u8)]
enum PublishProperty {
    PayloadFormatIndicator(PayloadFormatIndicator),
    MessageExpiryInterval(MessageExpiryInterval),
    TopicAlias(TopicAlias),
    ResponseTopic(ResponseTopic),
    CorrelationData(CorrelationData),
    UserProperty(UserProperty),
    SubscriptionIdentifier(SubscriptionIdentifier),
    ContentType(ContentType),
}

#[derive(Debug, PartialEq, Clone)]
enum PayloadFormatIndicator {
    UnspecifiedBytes,
    UTF8,
}

impl PayloadFormatIndicator {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        match buf[start_pos] {
            0x00 => Ok((PayloadFormatIndicator::UnspecifiedBytes, start_pos + 1)),
            0x01 => Ok((PayloadFormatIndicator::UTF8, start_pos + 1)),
            _ => Err(MqttError::InvalidFormat),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
struct MessageExpiryInterval(u32);
impl MessageExpiryInterval {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let i = u32::from_be_bytes(
            buf[start_pos..start_pos + 4]
                .try_into()
                .map_err(|_| MqttError::InvalidFormat)?,
        );
        return Ok((MessageExpiryInterval(i), start_pos + 4));
    }
}

#[derive(Debug, PartialEq, Clone)]
struct TopicAlias(u16);
impl TopicAlias {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let i = ((buf[start_pos] as u16) << 8) + (buf[start_pos + 1] as u16);
        if i == 0 {
            return Err(MqttError::InvalidFormat);
        }
        Ok((TopicAlias(i), start_pos + 2))
    }
}

/*
 サイズを返す場合は一貫して次の絶対位置で返す
*/
#[derive(Debug, PartialEq, Clone)]
struct ResponseTopic(String);
impl ResponseTopic {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let (res, pos) = decode_utf8_string(buf, start_pos)?;
        Ok((ResponseTopic(res), pos))
    }
}

#[derive(Debug, PartialEq, Clone)]
struct CorrelationData(bytes::Bytes);
impl CorrelationData {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        // [TODO] u16 length
        let length = ((buf[start_pos] as usize) << 8) + buf[start_pos + 1] as usize;
        // [TODO] length分だけコピー
        return Ok((
            CorrelationData(bytes::Bytes::copy_from_slice(
                &buf[start_pos + 2..start_pos + 2 + length],
            )),
            start_pos + 2 + length,
        ));
    }
}

#[derive(Debug, PartialEq, Clone)]
struct UserProperty((String, String));
impl UserProperty {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let (key, next_pos) = decode_utf8_string(buf, start_pos)?;
        let (value, next_pos) = decode_utf8_string(buf, next_pos)?;
        Ok((UserProperty((key, value)), next_pos))
    }
}

#[derive(Debug, PartialEq, Clone)]
struct SubscriptionIdentifier(u32);
impl SubscriptionIdentifier {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let i = u32::from_be_bytes(
            buf[start_pos..start_pos + 4]
                .try_into()
                .map_err(|_| MqttError::InvalidFormat)?,
        );
        return Ok((SubscriptionIdentifier(i), start_pos + 4));
    }
}

#[derive(Debug, PartialEq, Clone)]
struct ContentType(String);
impl ContentType {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let (res, pos) = decode_utf8_string(buf, start_pos)?;
        Ok((ContentType(res), pos))
    }
}

#[derive(PartialEq, Debug, Default, Clone, Copy)]
enum QoS {
    #[default]
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
            _ => Err(MqttError::InvalidFormat),
        }
    }
}

#[derive(PartialEq, Debug, Default, Clone)]
pub struct Connect {
    pub remain_length: usize,
    pub protocol_name: ProtocolName,
    pub protocol_ver: ProtocolVersion,
    pub connect_flags: ConnectFlags,
    pub keepalive: KeepAlive,
    pub properties: ConnectProperties,
    pub client_id: ClientId,
    pub username: Option<UserName>,
    pub password: Option<Password>,
    pub will_properties: WillProperties,
    pub will_topic: Option<TopicName>,
    pub will_payload: Option<WillPayload>,
}

#[derive(PartialEq, Debug, Default, Clone)]
struct ProtocolName(String);

impl ProtocolName {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let length = (((buf[start_pos] as usize) << 8) + (buf[start_pos + 1] as usize)) as usize;
        let ret = String::from_utf8(buf[start_pos + 2..start_pos + 2 + length].to_vec())
            .or_else(|_| Err(MqttError::InvalidFormat))?;
        Ok((ProtocolName(ret), start_pos + 2 + length))
    }
}

#[derive(PartialEq, Debug, Default, Clone, Copy)]
pub struct ProtocolVersion(u8);
impl ProtocolVersion {
    pub fn new(version: u8) -> Self {
        Self(version)
    }
    pub fn value(self) -> u8 {
        self.0
    }
    pub fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        Ok((ProtocolVersion(buf[start_pos]), start_pos + 1))
    }
    fn into_inner(self) -> u8 {
        self.0
    }
}

#[derive(PartialEq, Debug, Default, Clone, Copy)]
struct ConnectFlags {
    user_name_flag: bool,
    password_flag: bool,
    will_retain: bool,
    will_qos: QoS,
    will_flag: bool,
    clean_start: bool,
}
impl ConnectFlags {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        // The Server MUST validate that the reserved flag in the CONNECT
        // packet is set to 0 [MQTT-3.1.2-3].
        // If the reserved flag is not 0 it is a Malformed Packet.
        // Refer to section 4.13 for information about handling errors.
        if buf[start_pos] & 0b00000001 != 0 {
            return Err(MqttError::InvalidFormat);
        }
        let will_qos = match (buf[start_pos] & 0b00011000) >> 3 {
            0x00 => QoS::QoS0,
            0x01 => QoS::QoS1,
            0x02 => QoS::QoS2,
            _ => return Err(MqttError::InvalidFormat),
        };
        Ok((
            ConnectFlags {
                user_name_flag: buf[start_pos] & 0b10000000 != 0,
                password_flag: buf[start_pos] & 0b01000000 != 0,
                will_retain: buf[start_pos] & 0b00100000 != 0,
                will_qos,
                will_flag: buf[start_pos] & 0b00000100 != 0,
                clean_start: buf[start_pos] & 0b00000010 != 0,
            },
            start_pos + 1,
        ))
    }
}

#[derive(PartialEq, Debug, Default, Clone, Copy)]
struct KeepAlive(u16);
impl KeepAlive {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        Ok((
            KeepAlive(((buf[start_pos] as u16) << 8) + (buf[start_pos + 1] as u16)),
            start_pos + 2,
        ))
    }
    fn into_inner(self) -> u16 {
        self.0
    }
}

// Connect Properties

#[derive(PartialEq, Debug, Default, Clone, Copy)]
struct SessionExpiryInterval(u32);
impl SessionExpiryInterval {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let i = u32::from_be_bytes(
            buf[start_pos..start_pos + 4]
                .try_into()
                .map_err(|_| MqttError::InvalidFormat)?,
        );
        return Ok((SessionExpiryInterval(i), start_pos + 4));
    }
    fn into_inner(self) -> u32 {
        self.0
    }
}

#[derive(PartialEq, Debug, Default, Clone, Copy)]
struct ReceiveMaximum(u16);
impl ReceiveMaximum {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let i = ((buf[start_pos] as u16) << 8) + buf[start_pos + 1] as u16;
        if i == 0 {
            // It is a Protocol Error to include the Receive Maximum value more than once or for it to have the value 0.
            return Err(MqttError::InvalidFormat);
        }
        return Ok((ReceiveMaximum(i), start_pos + 2));
    }
    fn into_inner(self) -> u16 {
        self.0
    }
}

#[derive(PartialEq, Debug, Default, Clone, Copy)]
struct MaximumPacketSize(u32);
impl MaximumPacketSize {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let i = u32::from_be_bytes(
            buf[start_pos..start_pos + 4]
                .try_into()
                .map_err(|_| MqttError::InvalidFormat)?,
        );
        if i == 0 {
            return Err(MqttError::InvalidFormat);
        }
        return Ok((Self(i), start_pos + 4));
    }
    fn into_inner(self) -> u32 {
        self.0
    }
}

#[derive(PartialEq, Debug, Default, Clone, Copy)]
struct TopicAliasMaximum(u16);
impl TopicAliasMaximum {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let i = ((buf[start_pos] as u16) << 8) + buf[start_pos + 1] as u16;
        if i == 0 {
            // It is a Protocol Error to include the Receive Maximum value more than once or for it to have the value 0.
            return Err(MqttError::InvalidFormat);
        }
        return Ok((Self(i), start_pos + 2));
    }
    fn into_inner(self) -> u16 {
        self.0
    }
}

#[derive(PartialEq, Debug, Default, Clone, Copy)]
struct RequestResponseInformation(bool);
impl RequestResponseInformation {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let i = match buf[start_pos] {
            0x00 => false,
            0x01 => true,
            _ => {
                return Err(MqttError::InvalidFormat);
            }
        };
        return Ok((Self(i), start_pos + 1));
    }
    fn into_inner(self) -> bool {
        self.0
    }
}

#[derive(PartialEq, Debug, Default, Clone, Copy)]
struct RequestProblemInformation(bool);
impl RequestProblemInformation {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let i = match buf[start_pos] {
            0x00 => false,
            0x01 => true,
            _ => {
                return Err(MqttError::InvalidFormat);
            }
        };
        return Ok((Self(i), start_pos + 1));
    }
    fn into_inner(self) -> bool {
        self.0
    }
}

#[derive(PartialEq, Debug, Default, Clone)]
struct AuthenticationMethod(String);
impl AuthenticationMethod {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let (i, s) = decode_utf8_string(buf, start_pos)?;
        return Ok((Self(i), s));
    }
    fn into_inner(self) -> String {
        self.0
    }
}

#[derive(PartialEq, Debug, Default, Clone)]
struct AuthenticationData(bytes::Bytes);
impl AuthenticationData {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        // u16 length
        let length = ((buf[start_pos] as usize) << 8) + buf[start_pos + 1] as usize;
        // length分だけコピー
        return Ok((
            Self(bytes::Bytes::copy_from_slice(
                &buf[start_pos + 2..start_pos + 2 + length],
            )),
            start_pos + 2 + length,
        ));
    }
    fn into_inner(self) -> bytes::Bytes {
        self.0
    }
}

// Will Properties
#[derive(PartialEq, Debug, Default, Clone, Copy)]
struct WillDelayInterval(u32);
impl WillDelayInterval {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let i = u32::from_be_bytes(
            buf[start_pos..start_pos + 4]
                .try_into()
                .map_err(|_| MqttError::InvalidFormat)?,
        );
        return Ok((Self(i), start_pos + 4));
    }
    fn into_inner(self) -> u32 {
        self.0
    }
}

// Will Payload
#[derive(PartialEq, Debug, Default, Clone)]
struct WillPayload(bytes::Bytes);
impl WillPayload {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        // u16 length
        let length = ((buf[start_pos] as usize) << 8) + buf[start_pos + 1] as usize;
        // length分だけコピー
        return Ok((
            Self(bytes::Bytes::copy_from_slice(
                &buf[start_pos + 2..start_pos + 2 + length],
            )),
            start_pos + 2 + length,
        ));
    }
    fn into_inner(self) -> bytes::Bytes {
        self.0
    }
}

// User Name
#[derive(PartialEq, Debug, Default, Clone)]
struct UserName(String);
impl UserName {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let (i, s) = decode_utf8_string(buf, start_pos)?;
        return Ok((Self(i), s));
    }
    fn into_inner(self) -> String {
        self.0
    }
}

// Will Payload
#[derive(Debug, PartialEq, Clone)]
struct Password(bytes::Bytes);
impl Password {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        // u16 length
        let length = ((buf[start_pos] as usize) << 8) + buf[start_pos + 1] as usize;
        // length分だけコピー
        return Ok((
            Self(bytes::Bytes::copy_from_slice(
                &buf[start_pos + 2..start_pos + 2 + length],
            )),
            start_pos + 2 + length,
        ));
    }
    fn into_inner(self) -> bytes::Bytes {
        self.0
    }
}

#[derive(PartialEq, Debug, Default, Clone)]
pub struct ConnectProperties {
    pub session_expiry_interval: Option<SessionExpiryInterval>,
    pub receive_maximum: Option<ReceiveMaximum>,
    pub maximum_packet_size: Option<MaximumPacketSize>,
    pub topic_alias_maximum: Option<TopicAliasMaximum>,
    pub request_response_information: Option<RequestResponseInformation>,
    pub request_problem_information: Option<RequestProblemInformation>,
    pub user_properties: Option<Vec<UserProperty>>,
    pub authentication_method: Option<AuthenticationMethod>,
    pub authentication_data: Option<AuthenticationData>,
}
#[derive(PartialEq, Debug, Default, Clone)]
pub struct WillProperties {
    pub will_delay_interval: Option<WillDelayInterval>,
    pub payload_format_indicator: Option<PayloadFormatIndicator>,
    pub message_expiry_interval: Option<MessageExpiryInterval>,
    pub content_type: Option<ContentType>,
    pub response_topic: Option<ResponseTopic>,
    pub correlation_data: Option<CorrelationData>,
    pub user_properties: Option<Vec<UserProperty>>,
    pub subscription_identifier: Option<SubscriptionIdentifier>,
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

pub fn decode_u32_bytes(
    buf: &bytes::BytesMut,
    start_pos: usize,
) -> std::result::Result<u32, MqttError> {
    Ok(u32::from_be_bytes(
        buf[start_pos..start_pos + 4]
            .try_into()
            .map_err(|_| MqttError::InvalidFormat)?,
    ))
}

pub fn decode_u16_bytes(
    buf: &bytes::BytesMut,
    start_pos: usize,
) -> std::result::Result<u16, MqttError> {
    Ok(u16::from_be_bytes(
        buf[start_pos..start_pos + 2]
            .try_into()
            .map_err(|_| MqttError::InvalidFormat)?,
    ))
}

pub fn decode_u32_bytes_sliced(v: &[u8]) -> std::result::Result<u32, MqttError> {
    Ok(u32::from_be_bytes(
        v.try_into().map_err(|_| MqttError::InvalidFormat)?,
    ))
}

pub fn decode_utf8_string(
    buf: &bytes::BytesMut,
    start_pos: usize,
) -> Result<(String, usize), MqttError> {
    let length = decode_u16_bytes(buf, start_pos)? as usize;
    if start_pos + 2 + length > buf.len() {
        return Err(MqttError::InsufficientBytes.into());
    }

    let str = match std::str::from_utf8(&buf[start_pos + 2..start_pos + 2 + length]) {
        Ok(v) => v,
        Err(_) => return Err(MqttError::InvalidFormat),
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
        if buf.len() <= pos {
            return Err(MqttError::InsufficientBytes);
        }
        remaining_length += ((buf[pos] & 0b01111111) as usize) << (inc * 7);
        if (buf[pos] & 0b10000000) == 0 {
            return Ok((remaining_length, pos + 1)); // 次のバイト位置を返す
        }
        inc += 1;
    }
    Ok((remaining_length, start_pos + 4)) // 全4バイトを処理した場合も次の位置を返す
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

#[derive(PartialEq, Debug, Default, Clone)]
struct ClientId(String);
impl ClientId {
    fn try_from(
        buf: &bytes::BytesMut,
        start_pos: usize,
    ) -> std::result::Result<(Self, usize), MqttError> {
        let (res, pos) = decode_utf8_string(buf, start_pos)?;
        /*
        if !res.chars().all(|c| c.ids_ascii_alphanumeric()) {
            return Err(MqttError::InvalidFormat);
        }
         */
        Ok((Self(res), pos))
    }
    fn into_inner(self) -> String {
        self.0
    }
}

impl MqttPacket for Subscribe {
    fn decode_variable_header(
            &mut self,
            buf: &bytes::BytesMut,
            start_pos: usize,
            protocol_version: Option<ProtocolVersion>,
        ) -> Result<usize, MqttError> {
        // require protocol version
        let protocol_version = if let Some(v) = protocol_version {
            v
        } else {
            return Err(MqttError::Unexpected);
        };

        let mut next_pos;
        (self.packet_id, next_pos) = PacketId::try_from(buf, start_pos)?;
        if protocol_version.value() >= 5 {
            let property_length;
            (property_length, next_pos) = decode_variable_length(buf, next_pos)?;
            let end_pos = next_pos + property_length;
            // [TODO] protocol version
            loop {
                if next_pos == end_pos {
                    break;
                }
                if next_pos > end_pos {
                    return Err(MqttError::InvalidFormat);
                }
                match buf[next_pos] {
                    PROPERTY_USER_PROPERTY_ID => {
                        let user_property;
                        (user_property, next_pos) = UserProperty::try_from(buf, next_pos + 1)?;
                        // 所有権を奪わずに変更する。
                        self.sub_properties.user_properties
                            .get_or_insert_with(Vec::new)
                            .push(user_property);
                    }
                    PROPERTY_SUBSCRIPTION_IDENTIFIER_ID => {
                        let result;
                        (result, next_pos) = SubscriptionIdentifier::try_from(buf, next_pos + 1)?;
                        self.sub_properties.subscription_identifier = Some(result);
                    }
                    _ => {
                        return Err(MqttError::InvalidFormat);
                    }
                }
            }
        }

        self.payload_length = self.remain_length - (next_pos - start_pos);
        return Ok(next_pos);
    }
    
    fn decode_payload(
        &mut self,
        buf: &bytes::BytesMut,
        start_pos: usize,
        protocol_version: Option<ProtocolVersion>,
    ) -> Result<usize, MqttError> {
        // require protocol version
        let protocol_version = if let Some(v) = protocol_version {
            v
        } else {
            return Err(MqttError::Unexpected);
        };

        let mut next_pos = start_pos;
        /* guard for default */
        if self.payload_length == 0 {
            return Err(MqttError::Unexpected);
        }
        loop {
            let f: TopicFilter;
            let op: SubscribeOption;
            (f, next_pos) = TopicFilter::try_from(buf, next_pos)?;
            if protocol_version.value() >= 5 {
                (op, next_pos) = SubscribeOption::try_from(buf, next_pos)?;
                self.topic_filters.push((f, Some(op)));
            } else {
                self.topic_filters.push((f, None));
            }

            
            if self.payload_length == next_pos - start_pos {
                break
            } else if self.payload_length < next_pos - start_pos {
                return Err(MqttError::Unexpected);
            }
        }
        return Ok(next_pos);
    }
    
    fn encode_header(&self) -> Result<Bytes, MqttError> {
        todo!()
    }
    
    fn encode_payload_chunk(&self) -> Result<Option<Bytes>, MqttError> {
        todo!()
    }
}


impl MqttPacket for Publish {
    // next_positionを返す
    // すべて揃っている前提としない、なぜならば、Publishには大量のデータが転送される可能性があるので、
    // メモリを節約したいから！
    fn decode_variable_header(
        &mut self,
        buf: &bytes::BytesMut,
        start_pos: usize,
        protocol_version: Option<ProtocolVersion>,
    ) -> Result<usize, MqttError> {
        // topic name UTF-8 Encoded String
        let (result, mut next_pos) = TopicName::try_from(buf, start_pos)?;
        self.topic_name = Some(result);
        dbg!(&self.topic_name);
        // packet identifier
        if self.qos == QoS::QoS1 || self.qos == QoS::QoS2 {
            // check
            dbg!(self.qos);
            if buf.len() < next_pos + 1 {
                return Err(MqttError::InsufficientBytes.into());
            }
            let result;
            (result, next_pos) = PacketId::try_from(buf, next_pos)?;

            self.packet_id = Some(result);
        }
        // Properties
        let property_length;

        (property_length, next_pos) = decode_variable_length(buf, next_pos)?;

        // 9 | 10 11 12 13 14 | [ ]
        let end_pos = next_pos + property_length;
        loop {
            dbg!(next_pos, end_pos);
            if next_pos == end_pos {
                break;
            }
            if next_pos > end_pos {
                return Err(MqttError::InvalidFormat);
            }
            dbg!(format!("0x{:x}", buf[next_pos]));
            match buf[next_pos] {
                PROPERTY_PAYLOAD_FORMAT_INDICATOR_ID => {
                    let result;
                    (result, next_pos) = PayloadFormatIndicator::try_from(buf, next_pos + 1)?;
                    self.pub_properties.payload_format_indicator = Some(result);
                }
                PROPERTY_MESSAGE_EXPIRY_INTERVAL_ID => {
                    let result;
                    (result, next_pos) = MessageExpiryInterval::try_from(buf, next_pos + 1)?;
                    self.pub_properties.message_expiry_interval = Some(result);
                }
                PROPERTY_TOPIC_ALIAS_ID => {
                    // It is a Protocol Error to include the Topic Alias value more than once.
                    if self.pub_properties.topic_alias != None {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = TopicAlias::try_from(buf, next_pos + 1)?;
                    self.pub_properties.topic_alias = Some(result);
                }
                PROPERTY_RESPONSE_TOPIC_ID => {
                    // It is a Protocol Error to include the Topic Alias value more than once.
                    if self.pub_properties.response_topic != None {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = ResponseTopic::try_from(buf, next_pos + 1)?;
                    self.pub_properties.response_topic = Some(result);
                }
                PROPERTY_CORRELATION_DATA_ID => {
                    // It is a Protocol Error to include the Topic Alias value more than once.
                    if self.pub_properties.correlation_data != None {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = CorrelationData::try_from(buf, next_pos + 1)?;
                    self.pub_properties.correlation_data = Some(result);
                }
                PROPERTY_USER_PROPERTY_ID => {
                    let user_property;
                    (user_property, next_pos) = UserProperty::try_from(buf, next_pos + 1)?;
                    // 所有権を奪わずに変更する。
                    self.pub_properties
                        .user_properties
                        .get_or_insert_with(Vec::new)
                        .push(user_property);
                }
                PROPERTY_SUBSCRIPTION_IDENTIFIER_ID => {
                    let result;
                    (result, next_pos) = SubscriptionIdentifier::try_from(buf, next_pos + 1)?;
                    self.pub_properties
                        .subscription_identifier
                        .get_or_insert_with(Vec::new)
                        .push(result);
                }
                PROPERTY_CONTENT_TYPE_ID => {
                    // It is a Protocol Error to include the Content Type more than once.
                    if self.pub_properties.content_type != None {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = ContentType::try_from(buf, next_pos + 1)?;
                    self.pub_properties.content_type = Some(result);
                }
                _ => {
                    return Err(MqttError::InvalidFormat);
                }
            }
        }
        return Ok(next_pos);
    }

    fn decode_payload(
        &mut self,
        buf: &bytes::BytesMut,
        start_pos: usize,
        protocol_version: Option<ProtocolVersion>,
    ) -> Result<usize, MqttError> {
        Ok(0)
    }
    // [TODO]
    fn encode_header(&self) -> Result<Bytes, MqttError> {
        todo!()
    }
    fn encode_payload_chunk(&self) -> Result<Option<Bytes>, MqttError> {
        todo!()
    }
}



impl MqttPacket for Connect {
    // Connectはすべて揃っている前提でデコードする
    fn decode_variable_header(
        &mut self,
        buf: &bytes::BytesMut,
        start_pos: usize,
        protocol_version: Option<ProtocolVersion>,
    ) -> Result<usize, MqttError> {
        let (result, mut next_pos) = ProtocolName::try_from(buf, start_pos)?;
        self.protocol_name = result;
        (self.protocol_ver, next_pos) = ProtocolVersion::try_from(buf, next_pos)?;
        (self.connect_flags, next_pos) = ConnectFlags::try_from(buf, next_pos)?;
        (self.keepalive, next_pos) = KeepAlive::try_from(buf, next_pos)?;
        let property_length;
        dbg!(next_pos);
        // MQTT version5
        if self.protocol_ver.into_inner() != 0b00000101 {
            return Ok(next_pos);
        }
        (property_length, next_pos) = decode_variable_length(buf, next_pos)?;
        dbg!("property length ");
        dbg!(property_length);
        dbg!(next_pos);
        dbg!(format!("0x{:02x}", buf[next_pos]));
        dbg!(format!("0x{:02x}", buf[next_pos + 1]));

        let end_pos = next_pos + property_length;

        loop {
            dbg!(next_pos, end_pos);
            if next_pos == end_pos {
                break;
            }
            if next_pos > end_pos {
                return Err(MqttError::InvalidFormat);
            }
            dbg!(format!("0x{:x}", buf[next_pos]));
            match buf[next_pos] {
                PROPERTY_SESSION_EXPIRY_INTERVAL => {
                    if let Some(_) = self.properties.session_expiry_interval {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = SessionExpiryInterval::try_from(buf, next_pos + 1)?;
                    self.properties.session_expiry_interval = Some(result);
                }
                PROPERTY_RECIEVE_MAXIMUM => {
                    if let Some(_) = self.properties.receive_maximum {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = ReceiveMaximum::try_from(buf, next_pos + 1)?;
                    self.properties.receive_maximum = Some(result);
                }
                PROPERTY_MAXIMUM_PACKET_SIZE => {
                    if let Some(_) = self.properties.maximum_packet_size {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = MaximumPacketSize::try_from(buf, next_pos + 1)?;
                    self.properties.maximum_packet_size = Some(result);
                }
                PROPERTY_TOPIC_ALIAS_MAXIMUM => {
                    if let Some(_) = self.properties.topic_alias_maximum {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = TopicAliasMaximum::try_from(buf, next_pos + 1)?;
                    self.properties.topic_alias_maximum = Some(result);
                }
                PROPERTY_REQUEST_RESPONSE_INFORMATION => {
                    if let Some(_) = self.properties.request_response_information {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = RequestResponseInformation::try_from(buf, next_pos + 1)?;
                    self.properties.request_response_information = Some(result);
                }
                PROPERTY_REQUEST_PROBLEM_INFORMATION => {
                    if let Some(_) = self.properties.request_problem_information {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = RequestProblemInformation::try_from(buf, next_pos + 1)?;
                    self.properties.request_problem_information = Some(result);
                }
                PROPERTY_USER_PROPERTY_ID => {
                    let user_property;
                    (user_property, next_pos) = UserProperty::try_from(buf, next_pos + 1)?;
                    // 所有権を奪わずに変更する。
                    self.properties
                        .user_properties
                        .get_or_insert_with(Vec::new)
                        .push(user_property);
                }
                PROPERTY_AUTHENTICATION_METHOD => {
                    if let Some(_) = self.properties.authentication_method {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = AuthenticationMethod::try_from(buf, next_pos + 1)?;
                    self.properties.authentication_method = Some(result);
                }
                PROPERTY_AUTHENTICATION_DATA => {
                    if let Some(_) = self.properties.authentication_data {
                        return Err(MqttError::InvalidFormat);
                    }
                    let result;
                    (result, next_pos) = AuthenticationData::try_from(buf, next_pos + 1)?;

                    self.properties.authentication_data = Some(result);
                }
                _ => {
                    return Err(MqttError::InvalidFormat);
                }
            }

            // need to check authentication data
            // validation
            //"""It is a Protocol Error to include Authentication Data if there is no Authentication Method."""
            if (self.properties.authentication_data != None)
                && (self.properties.authentication_method == None)
            {
                return Err(MqttError::InvalidFormat);
            }
        }
        dbg!(&buf[next_pos]);
        dbg!(self);
        return Ok(next_pos);
    }

    fn decode_payload(
        &mut self,
        buf: &bytes::BytesMut,
        start_pos: usize,
        _protocol_version: Option<ProtocolVersion>,
    ) -> Result<usize, MqttError> {
        // Client Identifier(MUST)
        let mut next_pos;
        dbg!("decode_payload");
        dbg!(start_pos);
        dbg!(buf);
        (self.client_id, next_pos) = ClientId::try_from(buf, start_pos)?;
        dbg!(&self.client_id);
        if self.connect_flags.will_flag {
            // Will Properties
            // properties length

            let will_property_length;
            (will_property_length, next_pos) = decode_variable_length(buf, next_pos)?;
            let end_pos = next_pos + will_property_length;

            loop {
                dbg!(next_pos, end_pos);
                if next_pos == end_pos {
                    break;
                }
                if next_pos > end_pos {
                    return Err(MqttError::InvalidFormat);
                }
                dbg!(format!("0x{:x}", buf[next_pos]));
                match buf[next_pos] {
                    PROPERTY_WILL_DELAY_INTERVAL => {
                        if let Some(_) = self.will_properties.will_delay_interval {
                            return Err(MqttError::InvalidFormat);
                        }
                        let ret;
                        (ret, next_pos) = WillDelayInterval::try_from(buf, next_pos + 1)?;
                        self.will_properties.will_delay_interval = Some(ret);
                    }
                    PROPERTY_PAYLOAD_FORMAT_INDICATOR_ID => {
                        if let Some(_) = self.will_properties.payload_format_indicator {
                            return Err(MqttError::InvalidFormat);
                        }
                        let ret;
                        (ret, next_pos) = PayloadFormatIndicator::try_from(buf, next_pos + 1)?;
                        self.will_properties.payload_format_indicator = Some(ret);
                    }
                    PROPERTY_MESSAGE_EXPIRY_INTERVAL_ID => {
                        if let Some(_) = self.will_properties.message_expiry_interval {
                            return Err(MqttError::InvalidFormat);
                        }
                        let ret;
                        (ret, next_pos) = MessageExpiryInterval::try_from(buf, next_pos + 1)?;
                        self.will_properties.message_expiry_interval = Some(ret);
                    }
                    PROPERTY_CONTENT_TYPE_ID => {
                        if let Some(_) = self.will_properties.content_type {
                            return Err(MqttError::InvalidFormat);
                        }
                        let ret;
                        (ret, next_pos) = ContentType::try_from(buf, next_pos + 1)?;
                        self.will_properties.content_type = Some(ret);
                    }
                    PROPERTY_RESPONSE_TOPIC_ID => {
                        if let Some(_) = self.will_properties.response_topic {
                            return Err(MqttError::InvalidFormat);
                        }
                        let ret;
                        (ret, next_pos) = ResponseTopic::try_from(buf, next_pos + 1)?;
                        self.will_properties.response_topic = Some(ret);
                    }
                    PROPERTY_CORRELATION_DATA_ID => {
                        // It is a Protocol Error to include the Topic Alias value more than once.
                        if self.will_properties.correlation_data != None {
                            return Err(MqttError::InvalidFormat);
                        }
                        let result;
                        (result, next_pos) = CorrelationData::try_from(buf, next_pos + 1)?;
                        self.will_properties.correlation_data = Some(result);
                    }
                    PROPERTY_USER_PROPERTY_ID => {
                        let user_property;
                        (user_property, next_pos) = UserProperty::try_from(buf, next_pos + 1)?;
                        // 所有権を奪わずに変更する。
                        self.will_properties
                            .user_properties
                            .get_or_insert_with(Vec::new)
                            .push(user_property);
                    }
                    _ => {
                        return Err(MqttError::InvalidFormat);
                    }
                }
            }
            // Will Topic
            let ret;
            (ret, next_pos) = TopicName::try_from(buf, next_pos)?;
            self.will_topic = Some(ret);
            // Will Payload
            let ret;
            (ret, next_pos) = WillPayload::try_from(buf, next_pos)?;
            self.will_payload = Some(ret);
        }

        // User Name
        if self.connect_flags.user_name_flag {
            let ret;
            (ret, next_pos) = UserName::try_from(buf, next_pos)?;

            self.username = Some(ret);
            dbg!(&self.username);
        }
        // Password
        if self.connect_flags.password_flag {
            let ret;
            (ret, next_pos) = Password::try_from(buf, next_pos)?;
            self.password = Some(ret);
            dbg!(&self.password);
        }
        dbg!("all dne");

        return Ok(next_pos);
    }
    fn encode_header(&self) -> Result<Bytes, MqttError> {
        todo!()
    }

    fn encode_payload_chunk(&self) -> Result<Option<Bytes>, MqttError> {
        todo!()
    }
}
#[derive(Debug)]
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

pub mod decoder {
    use super::{
        decode_lower_fixed_header, decode_variable_length, Connect, ControlPacket, Disconnect, MqttError, MqttPacket, ProtocolVersion, Publish, Subscribe
    };
    // (, next_pos size)
    pub fn decode_fixed_header(
        buf: &bytes::BytesMut,
        start_pos: usize,
        _protocol_version: Option<ProtocolVersion>
    ) -> Result<(ControlPacket, usize), MqttError> {
        // 最初が0である前提
        match buf[0] >> 4 {
            0b0001 => {
                let (remaining_length, next_pos) = decode_variable_length(buf, start_pos + 1)?;
                return Result::Ok((
                    ControlPacket::CONNECT(Connect {
                        remain_length: remaining_length,
                        ..Default::default()
                    }),
                    next_pos,
                ));
            }
            0b1110 => {
                let (remaining_length, next_pos) = decode_variable_length(buf, start_pos + 1)?;
                return Result::Ok((
                    ControlPacket::DISCONNECT(Disconnect::new(remaining_length)),
                    next_pos,
                ));
            }
            // PUBLISH
            0b0011 => {
                let (dup, qos, retain) = decode_lower_fixed_header(buf, start_pos)?;
                let (remain_length, next_pos) = decode_variable_length(buf, start_pos + 1)?;
                return Ok((
                    ControlPacket::PUBLISH(Publish {
                        remain_length: remain_length,
                        qos: qos,
                        dup: dup,
                        retain: retain,
                        ..Default::default()
                    }),
                    next_pos,
                ));
            },
            // SUBSCRIBE
            0b1000 => {
                if buf[0] != 0b10000010 {
                    // decode error, must close
                    // MUST be set to 0,0,1 and 0 respectively. The Server MUST treat any other value as malformed and close the Network Connection
                    return Err(MqttError::InvalidFormat);
                }
                let (remain_length, next_pos) = decode_variable_length(buf, start_pos + 1)?;
                return Ok((
                    ControlPacket::SUBSCRIBE(Subscribe {
                        remain_length: remain_length,
                        ..Default::default()
                    })
                    ,next_pos
                ))
            }
            _control_type => return Err(MqttError::InvalidFormat),
        };
    }
}

#[cfg(test)]
mod tests {
    use bytes::Buf;

    use super::decoder::*;
    use crate::mqtt::*;
    use crate::mqtt::{self, ControlPacket};
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
    fn connect_minimum_decode() {
        let input = "101800044d5154540502003c00000b7075626c6973682d353439";
        let mut b = decode_hex(input);
        let ret = decode_fixed_header(&b, 0, Some(mqtt::ProtocolVersion(5)));
        assert!(ret.is_ok());
        let (packet, next_pos) = ret.unwrap();
        dbg!(next_pos);
        // | 0 1 2 3 | 4 5 6 7 |
        // 4から始めたい場合は4つ進める
        b.advance(next_pos);
        if let ControlPacket::CONNECT(mut connect) = packet {
            // variable header
            dbg!(format!("0x{:x}", b[0]));
            dbg!(format!("0x{:x}", b[1]));
            let ret = connect.decode_variable_header(&b, 0, Some(mqtt::ProtocolVersion(5)));
            assert!(ret.is_ok(), "Error: {}", ret.unwrap_err());
            match connect.protocol_ver.into_inner() {
                0x05 => {}
                _ => panic!("Protocol unmatch"),
            }
            assert_eq!(connect.connect_flags.clean_start, true);
            assert_eq!(connect.connect_flags.will_flag, false);
            assert_eq!(connect.connect_flags.will_qos, QoS::QoS0);
            assert_eq!(connect.connect_flags.will_retain, false);
            assert_eq!(connect.connect_flags.user_name_flag, false);
            assert_eq!(connect.connect_flags.password_flag, false);
            assert_eq!(connect.keepalive.clone().into_inner(), 60);
            let next_pos = ret.unwrap();
            println!("variable {}", next_pos);
            b.advance(next_pos);

            let res = connect.decode_payload(&b, 0, Some(mqtt::ProtocolVersion(5)));
            assert!(res.is_ok());
            assert_eq!(connect.client_id.into_inner(), "publish-549".to_string());
        } else {
            panic!("packet type error")
        }
    }
    #[test]
    fn mqtt_publish() {
        let input = "3328000b68656c6c6f2f746f70696300011123\
        000126000161000132260001630001337061796c6f6164";
        let mut b = decode_hex(input);
        let ret = decode_fixed_header(&b, 0, Some(mqtt::ProtocolVersion(5)));
        assert!(ret.is_ok());
        let (packet, consumed) = ret.unwrap();
        b.advance(consumed);
        if let ControlPacket::PUBLISH(mut publish) = packet {
            let ret = publish.decode_variable_header(&b, 0, Some(mqtt::ProtocolVersion(5)));
            assert!(ret.is_ok(), "Error: {}", ret.unwrap_err());
            let consumed = ret.unwrap();
            b.advance(consumed);
            let res = publish.decode_payload(&b, 0, Some(mqtt::ProtocolVersion(5)));
            assert_eq!(publish.qos, QoS::QoS1);
            assert_eq!(publish.retain, Retain(true));
            assert_eq!(
                publish.topic_name,
                Some(TopicName("hello/topic".to_string()))
            );
            assert_eq!(
                publish.pub_properties.user_properties,
                Some(vec![
                    UserProperty(("a".to_string(), "2".to_string())),
                    UserProperty(("c".to_string(), "3".to_string()))
                ])
            );
        }
    }

    // full publish test
    /*
    MQ Telemetry Transport Protocol, Publish Message
        Header Flags: 0x33, Message Type: Publish Message, QoS Level: At least once delivery (Acknowledged deliver), Retain
            0011 .... = Message Type: Publish Message (3)
            .... 0... = DUP Flag: Not set
            .... .01. = QoS Level: At least once delivery (Acknowledged deliver) (1)
            .... ...1 = Retain: Set
        Msg Len: 82
        Topic Length: 11
        Topic: hello/topic
        Message Identifier: 1
        Properties
            Total Length: 59
            ID: Payload Format Indicator (0x01)
            Value: 1
            ID: Publication Expiry Interval (0x02)
            Value: 60
            ID: Content Type (0x03)
            Length: 10
            Value: text/plain
            ID: Response Topic (0x08)
            Length: 14
            Value: response/topic
            ID: Correlation Data (0x09)
            Length: 5
            Value: 12345
            ID: User Property (0x26)
            Key Length: 1
            Key: a
            Value Length: 1
            Value: 2
            ID: User Property (0x26)
            Key Length: 1
            Key: c
            Value Length: 1
            Value: 3
        Message: payload


         */
    #[test]
    fn mqtt_publish_full() {
        let input = "3352000b68656c6c6f2f746f70696300013b01010\
        20000003c03000a746578742f706c61696e08000e726573706f6e7\
        3652f746f706963090005313233343526000161000132260001630\
        001337061796c6f6164";
        let mut b = decode_hex(input);
        let ret = decode_fixed_header(&b, 0, Some(mqtt::ProtocolVersion(5)));
        assert!(ret.is_ok());
        let (packet, consumed) = ret.unwrap();
        b.advance(consumed);
        if let ControlPacket::PUBLISH(mut publish) = packet {
            let ret = publish.decode_variable_header(&b, 0, Some(mqtt::ProtocolVersion(5)));
            assert!(ret.is_ok(), "Error: {}", ret.unwrap_err());
            let consumed = ret.unwrap();
            b.advance(consumed);
            let res = publish.decode_payload(&b, 0, Some(mqtt::ProtocolVersion(5)));
            assert_eq!(publish.qos, QoS::QoS1);
            assert_eq!(publish.retain, Retain(true));
            assert_eq!(
                publish.topic_name,
                Some(TopicName("hello/topic".to_string()))
            );
            assert_eq!(
                publish.pub_properties.user_properties,
                Some(vec![
                    UserProperty(("a".to_string(), "2".to_string())),
                    UserProperty(("c".to_string(), "3".to_string()))
                ])
            );
            assert_eq!(
                publish.pub_properties.message_expiry_interval,
                Some(MessageExpiryInterval(60))
            );
            assert_eq!(
                publish.pub_properties.payload_format_indicator,
                Some(PayloadFormatIndicator::UTF8)
            );
            assert_eq!(
                publish.pub_properties.content_type,
                Some(ContentType("text/plain".to_string()))
            );
            assert_eq!(
                publish.pub_properties.response_topic,
                Some(ResponseTopic("response/topic".to_string()))
            );
            assert_eq!(
                publish.pub_properties.correlation_data,
                Some(CorrelationData(bytes::Bytes::copy_from_slice(
                    "12345".as_bytes()
                )))
            )
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
        let ret = decode_fixed_header(&b, 0, Some(mqtt::ProtocolVersion(5)));
        assert!(ret.is_ok());
        let (packet, consumed) = ret.unwrap();
        println!("aaaaaa:{}", consumed);
        b.advance(consumed);
        if let ControlPacket::CONNECT(mut connect) = packet {
            let ret = connect.decode_variable_header(&b, 0, Some(mqtt::ProtocolVersion(5)));
            assert!(ret.is_ok(), "Error: {}", ret.unwrap_err());
            match connect.protocol_ver.into_inner() {
                0x05 => {}
                _ => panic!("Protocol unmatch"),
            }
            let next_pos = ret.unwrap();
            b.advance(next_pos);

            let res = connect.decode_payload(&b, 0, Some(mqtt::ProtocolVersion(5)));
            assert!(res.is_ok());
            assert_eq!(
                connect.client_id.clone().into_inner(),
                "publish-710".to_string()
            );
            println!(">>>> {:?}", connect);

            assert_eq!(connect.connect_flags.clean_start, true);
            assert_eq!(connect.connect_flags.will_flag, true);
            assert_eq!(connect.connect_flags.will_qos, QoS::QoS1);
            assert_eq!(connect.connect_flags.will_retain, false);
            assert_eq!(connect.connect_flags.user_name_flag, true);
            assert_eq!(connect.connect_flags.password_flag, true);
            assert_eq!(connect.keepalive.into_inner(), 60);

            assert_eq!(
                connect.properties.session_expiry_interval,
                Some(SessionExpiryInterval(120))
            );
            assert_eq!(connect.properties.receive_maximum, Some(ReceiveMaximum(20)));
            assert_eq!(
                connect.properties.maximum_packet_size,
                Some(MaximumPacketSize(256 * 1024))
            );
            assert_eq!(
                connect.properties.topic_alias_maximum,
                Some(TopicAliasMaximum(10))
            );
            assert_eq!(
                connect.properties.request_problem_information,
                Some(RequestProblemInformation(true))
            );
            assert_eq!(
                connect.properties.request_response_information,
                Some(RequestResponseInformation(true))
            );
            assert_eq!(
                connect.properties.user_properties,
                Some(vec![
                    UserProperty(("connect_key1".to_string(), "connect_value1".to_string())),
                    UserProperty(("connect_key2".to_string(), "connect_value2".to_string()))
                ])
            );
            assert_eq!(
                connect.properties.authentication_method,
                Some(AuthenticationMethod("example_auth_method".to_string()))
            );
            assert_eq!(
                connect.properties.authentication_data,
                Some(AuthenticationData(bytes::Bytes::copy_from_slice(
                    "example_auth_data".as_bytes()
                )))
            );
            /* will */
            assert_eq!(
                connect.will_properties.will_delay_interval,
                Some(WillDelayInterval(10))
            );
            assert_eq!(
                connect.will_properties.message_expiry_interval,
                Some(MessageExpiryInterval(30))
            );
            assert_eq!(
                connect.will_properties.content_type,
                Some(ContentType("text/plain".to_string()))
            );
            assert_eq!(
                connect.will_properties.payload_format_indicator,
                Some(PayloadFormatIndicator::UTF8)
            );
            assert_eq!(
                connect.will_properties.response_topic,
                Some(ResponseTopic("test/response".to_string()))
            );
            assert_eq!(
                connect.will_properties.correlation_data,
                Some(CorrelationData(bytes::Bytes::copy_from_slice(
                    "correlation_data_example".as_bytes()
                )))
            );
            assert_eq!(
                connect.will_properties.user_properties,
                Some(vec![
                    UserProperty(("key1".to_string(), "value1".to_string())),
                    UserProperty(("key2".to_string(), "value2".to_string())),
                    UserProperty(("key3".to_string(), "value3".to_string()))
                ])
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
            session_present: false,
            connect_reason: ConnackReason::Success,
            connack_properties: None,
            version: mqtt::ProtocolVersion(0x05),
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
            session_present: false,
            connect_reason: ConnackReason::NotAuthorized,
            connack_properties: None,
            version: mqtt::ProtocolVersion(0x05),
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
            session_present: false,
            connect_reason: ConnackReason::Success,
            connack_properties: Some(p),
            version: mqtt::ProtocolVersion(0x05),
        };

        let result_bytes = connack.build_bytes().unwrap();
        assert_eq!(result_bytes.as_ref(), expected);
    }
}
