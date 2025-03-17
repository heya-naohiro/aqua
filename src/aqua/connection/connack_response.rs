use std::fmt;

use mqtt_coder::mqtt;

pub struct ConnackResponse {
    pub session_present: bool,
    pub connack_properties: Option<mqtt::ConnackProperties>,
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
