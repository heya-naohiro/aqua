/*
use bytes::{Buf, BytesMut};
use futures_util::Stream;
use mqtt::ControlPacket;
use mqtt::{parser::parse_fixed_header, MqttError, MqttPacket};
use pin_project::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};
 */
mod mqtt;
/*

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
    fn push_data(&mut self, data: Vec<u8>) {}
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
        dbg!("poll next!!!");

        // ここのロジックを考える,図にする。
        // ----
        // bytesを待つ
        // headerのlengthがわかるまで待つ
        // headerを全部読むまで待つ
        // lengthが貯まるまで待つ or 流す
        let mut tmp_mqtt_packet: Option<ControlPacket> = None;
        let mut state = State::WaitFixedHeader;
        let mut variable_header_length = 0;
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
                            // return Poll::Pending;
                            return Poll::Pending;
                        } else {
                            return Poll::Ready(Some(Err(e)));
                        }
                    }
                },
                State::WaitVariableHeader => {
                    if let Some(ref mut packet) = tmp_mqtt_packet {
                        // bufferにリクエスト分だけデータがあるか確かめることはしない代わりにエラーをハンドルする
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
                                variable_header_length = advance_bytes;
                                state = State::WaitPayload;
                            }
                            Err(e) => {
                                if let Some(_insufficient) = e.downcast_ref::<MqttError>() {
                                    // Insufficientの場合は何もせずに非同期ランタイムに委ねる
                                    dbg!("downcast!!!");
                                    return Poll::Pending;
                                } else {
                                    return Poll::Ready(Some(Err(e)));
                                }
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
                    if let Some(ref mut packet) = tmp_mqtt_packet {
                        // bufferにリクエスト分だけデータがあるか確かめる
                        // 読むべきバイト数 > バッファーにあるバイト数
                        if packet.remain_length() - variable_header_length > this.buffer.len() {
                            return Poll::Pending;
                        }
                        let result = match packet {
                            mqtt::ControlPacket::CONNECT(connect) => {
                                connect.parse_payload(&this.buffer)
                            }
                            mqtt::ControlPacket::DISCONNECT(disconnect) => todo!(),
                            mqtt::ControlPacket::CONNACK(connack) => todo!(),
                        };
                        // Error handling
                        match result {
                            Ok(advance_bytes) => {
                                this.buffer.advance(advance_bytes);
                                state = State::WaitFixedHeader;
                            }
                            Err(e) => {
                                return Poll::Ready(Some(Err(e)));
                            }
                        }
                    } else {
                        // Error, internal error not found packet
                        return Poll::Ready(Some(Err(anyhow::anyhow!(
                            "internal error not found packet: WaitPayload"
                        ))));
                    }
                }
            }
        }
    }
}

// [TODO] テストを書く
// Streamのテスト？？？
#[cfg(test)]
mod tests {
    use std::io;

    use super::*;
    use async_stream::stream;
    use bytes::Bytes;
    use futures::{Stream, StreamExt};
    use hex::decode;
    use tokio;

    fn async_byte_stream(
        data: Vec<u8>,
        chunk_size: Vec<usize>,
    ) -> impl Stream<Item = Result<Bytes, io::Error>> {
        let byte = data;
        stream! {
            for chunk in byte.chunks(chunk_size) {
                yield Ok(Bytes::copy_from_slice(chunk));
            }
        }
    }
    fn decode_hex(str: &str) -> bytes::BytesMut {
        let decoded = hex::decode(str).unwrap();
        bytes::BytesMut::from(&decoded[..])
    }
    #[tokio::test]
    async fn connect_connect() {
        let mut input1 = decode_hex("101800044d5154540502003c00000b7075626c6973682d353439");
        let input2 = decode_hex(
            "10bd0200044d51545405ce003c7c110000007\
        81500136578616d706c655f617574685f6d6574686f6416001\
        16578616d706c655f617574685f64617461170119012100142\
        2000a26000c636f6e6e6563745f6b657931000e636f6e6e656\
        3745f76616c75653126000c636f6e6e6563745f6b657932000\
        e636f6e6e6563745f76616c7565322700040000000b7075626\
        c6973682d373130710101020000001e03000a746578742f706\
        c61696e08000d746573742f726573706f6e7365090018636f7\
        272656c6174696f6e5f646174615f6578616d706c651800000\
        00a2600046b657931000676616c7565312600046b657932000\
        676616c7565322600046b657933000676616c7565330009746\
        573742f77696c6c000c57696c6c206d657373616765000d796\
        f75725f757365726e616d65000d796f75725f70617373776f7264",
        );
        input1.extend_from_slice(&input2);
        let chunk_size = 10;
        let stream = async_byte_stream(input1.to_vec(), chunk_size);
        let stream = Box::pin(stream);
        let mut results = Vec::new();

        let mut mqtt_stream = MqttStream::new(stream);
        while let Some(item) = mqtt_stream.next().await {
            match item {
                Ok(value) => results.push(value),
                Err(e) => {
                    panic!("Error: {}", e);
                }
            }
        }
        // assert
    }
}
*/
