use std::task::{Context, Poll};

use bytes::{Buf, BytesMut};

use crate::mqtt::{self, ControlPacket, MqttError, MqttPacket};
use tracing::{instrument, trace};

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

    pub fn set_protocol_version(&mut self, v: Option<mqtt::ProtocolVersion>) {
        self.protocol_version = v;
    }

    pub fn poll_decode(
        &mut self,
        cx: &mut Context<'_>,
        mut buf: BytesMut,
    ) -> Poll<Result<(ControlPacket, BytesMut), (MqttError, BytesMut)>> {
        if buf.is_empty() {
            return Poll::Pending;
        }
        match &mut self.state {
            // next ( or first)
            DecoderState::Done => {
                trace!("New Decode start");
                // decode fixed header
                // packetは状態が変わるとムーブする、性能面で気になる場合はBox<ControlPacket>に変更する
                // [TODO] 後ほどの最適化でString->&strへの変更も 含めて？やる！！
                match mqtt::decoder::decode_fixed_header(&buf, 0, self.protocol_version) {
                    Ok(result) => {
                        let next_pos;
                        (self.tmp_packet, next_pos, self.remaining_length) = result;
                        buf.advance(next_pos);
                        cx.waker().wake_by_ref();
                        self.state = DecoderState::FixedHeaderDecoded;
                        return Poll::Pending(buf);
                    }
                    Err(err) => {
                        trace!("fixed header decode error {:?}", &buf);
                        return Poll::Ready(Err(err));
                    }
                }
            }
            DecoderState::FixedHeaderDecoded => {
                trace!("DecoderState::FixedHeaderDecoded");

                // decode variable header
                match self.tmp_packet.decode_variable_header(
                    &buf,
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
                trace!("DecoderState::VariableHeaderDecoded");
                // decode payload
                match self.tmp_packet.decode_payload(buf, self.protocol_version) {
                    Ok(buf) => {
                        trace!("DecoderState::VariableHeaderDecoded");
                        cx.waker().wake_by_ref();
                        self.state = DecoderState::Done;
                        return Poll::Ready(Ok((std::mem::take(&mut self.tmp_packet), buf)));
                    }
                    Err(err) => {
                        return Poll::Ready(Err(err));
                    }
                }
            }
        }
    }
}
