use std::task::{Context, Poll};

use bytes::{Buf, BytesMut};

use crate::mqtt::{self, ControlPacket, MqttError, MqttPacket};
use tracing::trace;

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
    pub buf: BytesMut,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            state: DecoderState::Done,
            tmp_packet: ControlPacket::UNDEFINED,
            protocol_version: None,
            remaining_length: 0,
            buf: BytesMut::new(),
        }
    }

    pub fn set_protocol_version(&mut self, v: Option<mqtt::ProtocolVersion>) {
        self.protocol_version = v;
    }

    pub fn poll_decode(&mut self, cx: &mut Context<'_>) -> Poll<Result<ControlPacket, MqttError>> {
        if self.buf.is_empty() {
            return Poll::Pending;
        }
        match &mut self.state {
            // next ( or first)
            DecoderState::Done => {
                trace!("New Decode start {:?}", self.buf);
                match mqtt::decoder::decode_fixed_header(&self.buf, 0, self.protocol_version) {
                    Ok(result) => {
                        let next_pos;
                        (self.tmp_packet, next_pos, self.remaining_length) = result;
                        self.buf.advance(next_pos);
                        cx.waker().wake_by_ref();
                        self.state = DecoderState::FixedHeaderDecoded;
                        return Poll::Pending;
                    }
                    Err(err) => {
                        trace!("fixed header decode error {:?}", &self.buf);
                        return Poll::Ready(Err(err));
                    }
                }
            }
            DecoderState::FixedHeaderDecoded => {
                trace!("DecoderState::FixedHeaderDecoded");

                // decode variable header
                match self.tmp_packet.decode_variable_header(
                    &self.buf,
                    0,
                    self.remaining_length,
                    self.protocol_version,
                ) {
                    Ok(size) => {
                        self.buf.advance(size);
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
                let buf = std::mem::take(&mut self.buf);
                match self.tmp_packet.decode_payload(buf, self.protocol_version) {
                    Ok(remaining_buf) => {
                        cx.waker().wake_by_ref();
                        self.state = DecoderState::Done;
                        self.buf = remaining_buf;
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
