use bytes::{Buf, BytesMut};
use futures_util::Stream;
use mqtt::ControlPacket;
use mqtt::{parser::parse_fixed_header, MqttError, MqttPacket};
use pin_project::pin_project;
use std::any;
use std::pin::Pin;
use std::task::{Context, Poll};
mod mqtt;

//元の(tcp)Streamを受け取ってMQTTStreamに変換する

// output: definition of stream
type MqttStreamResult = Result<mqtt::ControlPacket, anyhow::Error>;

// input
type StreamItem = Result<bytes::Bytes, std::io::Error>;

enum State {
    WaitFixedHeader,
    WaitVariableHeader,
    WaitPayload,
}

#[pin_project]
struct MqttStream<S>
where
    S: Stream<Item = StreamItem> + Unpin,
{
    #[pin]
    stream: S,
    buffer: BytesMut,
}

impl<S> MqttStream<S>
where
    S: Stream<Item = StreamItem> + Unpin,
{
    fn new(stream: S) -> Self {
        MqttStream {
            stream,
            buffer: BytesMut::new(),
        }
    }
}

//続き https://synamon.hatenablog.com/entry/rust_stream_entry
impl<S> Stream for MqttStream<S>
where
    S: Stream<Item = StreamItem> + Unpin,
{
    type Item = MqttStreamResult;
    fn poll_next(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut this = self.project();
        // ここのロジックを考える,図にする。
        // ----
        // bytesを待つ
        // headerのlengthがわかるまで待つ
        // headerを全部読むまで待つ
        // lengthが貯まるまで待つ or 流す
        let mut tmp_mqtt_packet: Option<ControlPacket> = None;
        let mut state = State::WaitFixedHeader;
        let mut request_buffer = 0;
        loop {
            // あるだけ読み込む
            match this.stream.as_mut().poll_next(cx) {
                Poll::Ready(Some(Ok(chunk))) => this.buffer.extend(&chunk),
                Poll::Ready(Some(Err(error))) => {
                    return Poll::Ready(Some(Err(anyhow::anyhow!(error))));
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
                // Streamの終端
                Poll::Ready(None) => {
                    if this.buffer.is_empty() {
                        return Poll::Ready(None);
                    }
                    // bufferに残っている場合は処理をする
                }
            }

            // stateの状態に応じて足りなければスキップcontinueする
            match state {
                State::WaitFixedHeader => match parse_fixed_header(&this.buffer) {
                    Ok((mqttpacket, fixed_header_length)) => {
                        tmp_mqtt_packet = Some(mqttpacket);
                        state = State::WaitVariableHeader;
                        this.buffer.advance(fixed_header_length);
                    }
                    Err(e) => {
                        // [TODO] Errorの洗練
                        if let Some(_insufficient) = e.downcast_ref::<MqttError>() {
                            // warning
                            continue;
                        } else {
                            return Poll::Ready(Some(Err(e)));
                        }
                    }
                },
                State::WaitVariableHeader => {
                    if let Some(ref mut packet) = tmp_mqtt_packet {
                        // bufferにリクエスト分だけデータがあるか確かめる
                        if packet.remain_length() > this.buffer.len() {
                            return Poll::Pending;
                        }
                        let result = match packet {
                            mqtt::ControlPacket::CONNECT(connect) => {
                                connect.parse_variable_header(&this.buffer)
                            }
                            mqtt::ControlPacket::DISCONNECT(disconnect) => todo!(),
                            mqtt::ControlPacket::CONNACK(connack) => todo!(),
                        };
                        // Error handling
                        match result {
                            Ok(advance_bytes) => {
                                this.buffer.advance(advance_bytes);
                            }
                            Err(e) => {
                                return Poll::Ready(Some(Err(anyhow::anyhow!(e))));
                            }
                        }
                    } else {
                        // Error, internal error not found packet
                        return Poll::Ready(Some(Err(anyhow::anyhow!(
                            "internal error not found packet: WaitVariableHeader"
                        ))));
                    }
                }
                State::WaitPayload => {
                    // bufferにリクエスト分だけデータがあるか確かめるかどうかはパケットの種類次第だと思う
                }
            }
        }
    }
}
