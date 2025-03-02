pub mod request;
pub mod response;

use bytes::Buf;
use bytes::BytesMut;
use mqtt_coder::decoder;
use mqtt_coder::encoder;
use mqtt_coder::mqtt;
use mqtt_coder::mqtt::ControlPacket;
use mqtt_coder::mqtt::MqttError;
use request::Request;
use response::Response;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::io::poll_read_buf;
use tower::Service;
const BUFFER_CAPACITY: usize = 4096;

pub struct Connection<S, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Future: Unpin, // `S::Future` を `Unpin` にする
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    service: S,
    io: IO,
    state: ConnectionState<S::Future>,
    buffer: BytesMut,
    write_buffer: BytesMut,
    decoder: decoder::Decoder,
    encoder: encoder::Encoder,
}

#[derive(Default)]
enum ConnectionState<F> {
    #[default]
    PreConnection,
    ReadingPacket,
    ProcessingService(F),
    WritingPacket(Response),
    Closed,
}

impl<S, IO> Connection<S, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin + 'static,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(service: S, io: IO) -> Self {
        Connection {
            service,
            io,
            state: ConnectionState::PreConnection,
            buffer: BytesMut::with_capacity(BUFFER_CAPACITY), /* [TODO] limit */
            write_buffer: BytesMut::new(),
            decoder: decoder::Decoder::new(),
            encoder: encoder::Encoder::new(),
        }
    }
}

impl<S, IO> Future for Connection<S, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Output = Result<(), Box<dyn std::error::Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.as_mut();
        let new_state;
        match std::mem::take(&mut this.state) {
            ConnectionState::PreConnection => {
                let _req = match this.as_mut().read_packet(cx) {
                    Poll::Ready(Ok(req)) => {
                        if let ControlPacket::CONNECT(_) = req.body {
                            req
                        } else {
                            return Poll::Ready(Err(Box::new(MqttError::Unexpected)));
                        }
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                };
                new_state = Some(ConnectionState::ReadingPacket);
            }
            // 要求をReadするフェーズ
            ConnectionState::ReadingPacket => {
                let req = match this.as_mut().read_packet(cx) {
                    Poll::Ready(Ok(req)) => req,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                };
                if let ControlPacket::DISCONNECT(_) = req.body {
                    // -> DropでClose処理を行う
                    return Poll::Ready(Ok(()));
                }
                let fut = this.service.call(req);
                new_state = Some(ConnectionState::ProcessingService(fut));
            }
            ConnectionState::ProcessingService(ref mut fut) => {
                let response = match Pin::new(fut).poll(cx) {
                    Poll::Ready(Ok(res)) => res, // Response を取得
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                    Poll::Pending => return Poll::Pending,
                };
                new_state = Some(ConnectionState::WritingPacket(response));
            }
            ConnectionState::WritingPacket(ref res) => match this.as_mut().write_packet(cx, res) {
                Poll::Ready(Ok(())) => {
                    new_state = Some(ConnectionState::ReadingPacket);
                }
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            },
            ConnectionState::Closed => {
                return Poll::Ready(Ok(()));
            }
        }
        if let Some(state) = new_state {
            this.state = state;
        }
        Poll::Pending
    }
}

impl<S, IO> Connection<S, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    fn read_packet(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Request<mqtt::ControlPacket>, Box<dyn std::error::Error>>> {
        // bufferにアクセスするためにPin<&mut Self> から &mut Self を取り出す
        let this = self.get_mut();
        let mut buf = &mut this.buffer;
        match poll_read_buf(Pin::new(&mut this.io), cx, &mut buf) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(0)) => {
                return Poll::Ready(Err("Connection closed".into()));
            }
            Poll::Ready(Ok(_n)) => match this.decoder.poll_decode(cx, &mut buf) {
                Poll::Ready(Ok(control_packet)) => Poll::Ready(Ok(Request::new(control_packet))),
                Poll::Ready(Err(e)) => return Poll::Ready(Err(Box::new(e))),
                Poll::Pending => return Poll::Pending,
            },
            Poll::Ready(Err(e)) => Poll::Ready(Err(Box::new(e))),
        }
    }
    // ここをloopなしにする
    fn write_packet(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        res: &Response,
    ) -> Poll<Result<(), Box<dyn std::error::Error>>> {
        // as_mut() は Pin<&mut Self> のまま再取得する。
        let mut this = self.as_mut();

        // `encoder` と `write_buffer` への参照を取得
        let encoder = &mut this.encoder;
        let write_buffer = &mut this.write_buffer;
        match encoder.poll_encode(cx, &res.packet, write_buffer) {
            Poll::Ready(Ok(Some(()))) => {
                while !this.buffer.is_empty() {
                    match Pin::new(&mut this.io).poll_write(cx, &this.write_buffer) {
                        Poll::Ready(Ok(n)) => {
                            if n == 0 {
                                return Poll::Ready(Err("Connection closed during write".into()));
                            }
                            this.buffer.advance(n); // 書き込んだ分をバッファから削除
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(Box::new(e))),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                //まだ続く、Encode起因のPending
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Poll::Ready(Ok(None)) => {
                // エンコードが完了
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)), // エラー
            Poll::Pending => Poll::Pending,             // エンコード待ち
        }
        // [TODO] 必ずbufferは終わったらclearする
    }
}
