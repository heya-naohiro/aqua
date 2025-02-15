use std::task::{Context, Poll};

use bytes::BytesMut;

use crate::mqtt::{ControlPacket, MqttPacket};

pub struct Encoder {
    state: EncodeState,
    buffer: BytesMut,
}

enum EncodeState {
    FixedHeader,
    VariableHeader,
    Payload,
    Done,
}

impl Encoder {
    pub fn new() -> Self {
        Encoder {
            state: EncodeState::FixedHeader,
            buffer: BytesMut::new(),
        }
    }
    pub fn reset(&mut self) {
        self.state = EncodeState::FixedHeader;
        self.buffer.clear();
    }

    pub fn poll_encode(
        &mut self,
        cx: &mut Context<'_>,
        packet: &ControlPacket,
    ) -> Poll<Option<Result<BytesMut, Box<dyn std::error::Error>>>> {
        loop {
            match self.state {
                EncodeState::FixedHeader => {
                    let fixed_header = packet.encode_fixed_header()?;
                    self.buffer.extend_from_slice(&fixed_header);
                    self.state = EncodeState::VariableHeader;
                }
                EncodeState::VariableHeader => {
                    let variable_header = packet.encode_variable_header()?;
                    self.buffer.extend_from_slice(&variable_header);
                    self.state = EncodeState::Payload;
                }
                EncodeState::Payload => {
                    if let Some(chunk) = packet.encode_payload_chunk()? {
                        self.buffer.extend_from_slice(&chunk);
                        return Poll::Ready(Some(Ok(self.buffer.split())));
                    } else {
                        self.state = EncodeState::Done;
                    }
                }
                EncodeState::Done => return Poll::Ready(None),
            }
        }
    }
}
