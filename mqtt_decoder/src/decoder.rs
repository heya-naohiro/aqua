use std::task::{Context, Poll};

use bytes::{Buf, BytesMut};

use crate::mqtt::{self, ControlPacket, MqttError, MqttPacket};

enum DecoderState {
    FixedHeaderDecoded,
    VariableHeaderDecoded,
    Done,
}

pub struct Decoder {
    state: DecoderState,
    tmp_packet: ControlPacket,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            state: DecoderState::Done,
            tmp_packet: ControlPacket::UNDEFINED,
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
        match &mut self.state {
            // next ( or first)
            DecoderState::Done => {
                // decode fixed header
                // packetは状態が変わるとムーブする、性能面で気になる場合はBox<ControlPacket>に変更する
                // [TODO] 後ほどの最適化でString->&strへの変更も 含めて？やる！！
                match mqtt::decoder::decode_fixed_header(buf, 0) {
                    Ok(result) => {
                        let next_pos;
                        (self.tmp_packet, next_pos) = result;
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
                // decode variable header
                match self.tmp_packet.decode_variable_header(buf, 0) {
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
                match self.tmp_packet.decode_payload(buf, 0) {
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
