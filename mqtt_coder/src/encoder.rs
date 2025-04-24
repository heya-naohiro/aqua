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

    pub fn poll_encode(
        &mut self,
        _cx: &mut Context<'_>,
        packet: ControlPacket,
        buffer: &mut BytesMut,
    ) -> Poll<Result<Option<()>, Box<dyn std::error::Error>>> {
        dbg!("poll encode", &self.state);
        match self.state {
            EncodeState::Header => {
                dbg!("fixed header");
                /* ここ！！このさきが実装されていない */
                let fixed_header = packet.encode_header()?;
                dbg!(&fixed_header);
                buffer.extend_from_slice(&fixed_header);
                self.state = EncodeState::Payload;
                Poll::Ready(Ok(Some(())))
            }
            EncodeState::Payload => {
                if let Some(chunk) = packet.encode_payload_chunk()? {
                    dbg!(&chunk);
                    buffer.extend_from_slice(&chunk);
                    return Poll::Ready(Ok(Some(())));
                } else {
                    /* Payloadが存在しないケース(Connack)がある */
                    self.state = EncodeState::Done;
                    dbg!("Done");
                    return Poll::Ready(Ok(None));
                }
            }
            EncodeState::Done => return Poll::Ready(Ok(None)),
        }
    }
}
