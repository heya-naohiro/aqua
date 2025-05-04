use std::task::{Context, Poll};

use bytes::BytesMut;

use crate::mqtt::{ControlPacket, MqttPacket};

#[derive(Debug)]
pub struct Encoder {
    state: EncodeState,
}
#[derive(Debug)]
enum EncodeState {
    Header, /* fixed headerとvariable headerは不可分、 fixed headerにremain lengthを含むため */
    Payload,
    Done,
}

impl Encoder {
    pub fn new() -> Self {
        Encoder {
            state: EncodeState::Header,
        }
    }
    pub fn encode_all(
        &mut self,
        packet: &ControlPacket,
        buffer: &mut BytesMut,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fixed_header = packet.encode_header()?;
        buffer.extend_from_slice(&fixed_header);
        if let Some(payload) = packet.encode_payload_chunk()? {
            buffer.extend_from_slice(&payload);
        }
        Ok(())
    }

    pub fn poll_encode(
        &mut self,
        _cx: &mut Context<'_>,
        packet: &ControlPacket,
        buffer: &mut BytesMut,
    ) -> Poll<Result<Option<()>, Box<dyn std::error::Error>>> {
        match self.state {
            EncodeState::Header => {
                let fixed_header = packet.encode_header()?;
                buffer.extend_from_slice(&fixed_header);
                self.state = EncodeState::Payload;
                Poll::Ready(Ok(Some(())))
            }
            EncodeState::Payload => {
                if let Some(chunk) = packet.encode_payload_chunk()? {
                    buffer.extend_from_slice(&chunk);
                    return Poll::Ready(Ok(Some(())));
                } else {
                    /* Payloadが存在しないケース(Connack)がある */
                    self.state = EncodeState::Done;
                    return Poll::Ready(Ok(None));
                }
            }
            EncodeState::Done => return Poll::Ready(Ok(None)),
        }
    }
}
