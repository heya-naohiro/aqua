use mqtt_coder::mqtt;
use std::convert::From;
use std::fmt;

#[derive(Default, Debug)]
pub struct ConnackResponse {
    pub session_present: bool,
    pub connack_properties: Option<mqtt::ConnackProperties>,
}

impl ConnackResponse {
    pub fn to_connack(self) -> mqtt::Connack {
        mqtt::Connack {
            remaining_length: 0,
            session_present: self.session_present,
            connect_reason: mqtt::ConnackReason::Success,
            connack_properties: self.connack_properties.map(|prop| prop.into()),
        }
    }
}

impl From<mqtt::Connack> for ConnackResponse {
    fn from(item: mqtt::Connack) -> Self {
        ConnackResponse {
            session_present: item.session_present,
            connack_properties: item.connack_properties.map(|prop| prop.into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConnackError {
    pub reason_code: mqtt::ConnackReason,
}

impl fmt::Display for ConnackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CONNACK error: {:?}", self.reason_code)
    }
}

impl std::error::Error for ConnackError {}

impl From<std::io::Error> for ConnackError {
    fn from(err: std::io::Error) -> Self {
        // 必要に応じた変換処理を実装
        ConnackError {
            reason_code: mqtt::ConnackReason::UnspecifiedError,
        }
    }
}

impl From<ConnackError> for std::io::Error {
    fn from(err: ConnackError) -> Self {
        std::io::Error::new(std::io::ErrorKind::Other, err.to_string())
    }
}
