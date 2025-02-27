use std::task::{Context, Poll};

use bytes::BytesMut;

use crate::mqtt::{ControlPacket, MqttPacket};

pub struct Encoder {
    state: EncodeState,
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
        }
    }

    pub fn poll_encode(
        &mut self,
        cx: &mut Context<'_>,
        packet: &ControlPacket,
        buffer: &mut BytesMut,
    ) -> Poll<Result<Option<()>, Box<dyn std::error::Error>>> {
        match self.state {
            EncodeState::FixedHeader => {
                let fixed_header = packet.encode_fixed_header()?;
                buffer.extend_from_slice(&fixed_header);
                self.state = EncodeState::VariableHeader;
                Poll::Ready(Ok(Some(())))
            }
            EncodeState::VariableHeader => {
                let variable_header = packet.encode_variable_header()?;
                buffer.extend_from_slice(&variable_header);
                self.state = EncodeState::Payload;

                Poll::Ready(Ok(Some(())))
            }
            EncodeState::Payload => {
                if let Some(chunk) = packet.encode_payload_chunk()? {
                    buffer.extend_from_slice(&chunk);
                    return Poll::Ready(Ok(Some(())));
                } else {
                    self.state = EncodeState::Done;
                    return Poll::Ready(Ok(None));
                }
            }
            EncodeState::Done => return Poll::Ready(Ok(None)),
        }
    }
}
