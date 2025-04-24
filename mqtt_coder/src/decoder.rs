use std::task::{Context, Poll};

use bytes::{Buf, BytesMut};

use crate::mqtt::{self, ControlPacket, MqttError, MqttPacket};

#[derive(Debug)]
enum DecoderState {
    FixedHeaderDecoded,
    VariableHeaderDecoded,
    Done,
}

pub struct Decoder {
    state: DecoderState,
    tmp_packet: ControlPacket,
    protocol_version: Option<mqtt::ProtocolVersion>,
    remaining_length: usize,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            state: DecoderState::Done,
            tmp_packet: ControlPacket::UNDEFINED,
            protocol_version: None,
            remaining_length: 0,
        }
    }
    pub fn poll_decode(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut BytesMut,
    ) -> Poll<Result<ControlPacket, MqttError>> {
        if buf.is_empty() {
            return Poll::Pending;
        }
        dbg!("poll_decode");
        dbg!(&self.state);
        dbg!(&buf);

        match &mut self.state {
            // next ( or first)
            DecoderState::Done => {
                // decode fixed header
                // packetは状態が変わるとムーブする、性能面で気になる場合はBox<ControlPacket>に変更する
                // [TODO] 後ほどの最適化でString->&strへの変更も 含めて？やる！！
                match mqtt::decoder::decode_fixed_header(buf, 0, self.protocol_version) {
                    Ok(result) => {
                        dbg!("fixed OK");
                        let next_pos;
                        (self.tmp_packet, next_pos, self.remaining_length) = result;
                        buf.advance(next_pos);
                        cx.waker().wake_by_ref();
                        self.state = DecoderState::FixedHeaderDecoded;
                        return Poll::Pending;
                    }
                    Err(err) => {
                        return Poll::Ready(Err(err));
                    }
                }
            }
            DecoderState::FixedHeaderDecoded => {
                dbg!("FixedHeaderDecoded");
                // decode variable header
                match self.tmp_packet.decode_variable_header(
                    buf,
                    0,
                    self.remaining_length,
                    self.protocol_version,
                ) {
                    Ok(size) => {
                        buf.advance(size);
                        cx.waker().wake_by_ref();
                        self.state = DecoderState::VariableHeaderDecoded;
                        return Poll::Pending;
                    }
                    Err(err) => {
                        return Poll::Ready(Err(err));
                    }
                }
            }
            DecoderState::VariableHeaderDecoded => {
                // decode payload
                match self
                    .tmp_packet
                    .decode_payload(buf, 0, self.protocol_version)
                {
                    Ok(size) => {
                        buf.advance(size);
                        cx.waker().wake_by_ref();
                        self.state = DecoderState::Done;
                        return Poll::Ready(Ok(std::mem::take(&mut self.tmp_packet)));
                    }
                    Err(err) => {
                        return Poll::Ready(Err(err));
                    }
                }
            }
        }
    }
}
