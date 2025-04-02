pub mod connack_response;
pub mod request;
pub mod response;

use bytes::Buf;
use bytes::BytesMut;
use hex::encode;
use mqtt_coder::decoder;
use mqtt_coder::encoder;
use mqtt_coder::mqtt;
use mqtt_coder::mqtt::ControlPacket;
use mqtt_coder::mqtt::MqttError;
use pin_project::pin_project;
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

#[pin_project]
pub struct Connection<S, CS, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Future: Unpin, // `S::Future` を `Unpin` にする
    CS: Service<
            Request<ControlPacket>,
            Response = connack_response::ConnackResponse,
            Error = connack_response::ConnackError,
        > + Unpin,
    CS::Future: Unpin, // `S::Future` を `Unpin` にする
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    service: S,
    connect_service: CS,
    #[pin]
    io: IO,
    state: ConnectionState<S::Future, CS::Future>,
    buffer: BytesMut,
    write_buffer: BytesMut,
    decoder: decoder::Decoder,
    encoder: encoder::Encoder,
}

#[derive(Default)]
enum ConnectionState<F, CF> {
    #[default]
    PreConnection,
    ProcessingConnect(Pin<Box<CF>>),
    ResponseConnect(connack_response::ConnackResponse),
    ReadingPacket,
    ProcessingService(Pin<Box<F>>),
    WritingPacket(Response),
    Closed,
}

impl<S, CS, IO> Connection<S, CS, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin + 'static,
    CS: Service<
            Request<ControlPacket>,
            Response = connack_response::ConnackResponse,
            Error = connack_response::ConnackError,
        > + Unpin,
    CS::Error: std::error::Error + Send + Sync + 'static,
    CS::Future: Unpin + 'static,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(service: S, connect_service: CS, io: IO) -> Self {
        Connection {
            service,
            connect_service,
            io,
            state: ConnectionState::PreConnection,
            buffer: BytesMut::with_capacity(BUFFER_CAPACITY), /* [TODO] limit */
            write_buffer: BytesMut::new(),
            decoder: decoder::Decoder::new(),
            encoder: encoder::Encoder::new(),
        }
    }
}

impl<S, CS, IO> Future for Connection<S, CS, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin,
    CS: Service<
            Request<ControlPacket>,
            Response = connack_response::ConnackResponse,
            Error = connack_response::ConnackError,
        > + Unpin,
    CS::Error: std::error::Error + Send + Sync + 'static,
    CS::Future: Unpin,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Output = Result<(), Box<dyn std::error::Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.as_mut();
        let mut new_state = Some(ConnectionState::PreConnection);
        match std::mem::take(&mut this.state) {
            // ここはConnectパケットを受け経ったかどうかのみ：接続管理
            ConnectionState::PreConnection => {
                dbg!("preConnection");
                let req = match this.as_mut().read_packet(cx) {
                    Poll::Ready(Ok(req)) => {
                        if let ControlPacket::CONNECT(_) = req.body {
                            req
                        } else {
                            return Poll::Ready(Err(Box::new(MqttError::Unexpected)));
                        }
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending, /* Pendingで終わるため、new_stateが変わらない */
                };
                let fut = Box::pin((this.connect_service).call(req));
                new_state = Some(ConnectionState::ProcessingConnect(fut));
            }
            ConnectionState::ProcessingConnect(mut fut) => {
                dbg!("processingConnaect");
                let connect_service_result = match fut.as_mut().poll(cx) {
                    Poll::Ready(Ok(res)) => res, // Response を取得
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                    Poll::Pending => return Poll::Pending,
                };
                dbg!("processingConnaect, new_state");

                new_state = Some(ConnectionState::ResponseConnect(connect_service_result));
            }
            ConnectionState::ResponseConnect(res) => {
                dbg!("responseConnect");
                match this.as_mut().write_connack(cx, res) {
                    Poll::Ready(Ok(_)) => (), // Response を取得
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                    Poll::Pending => return Poll::Pending,
                }
            }
            // 要求をReadするフェーズ
            ConnectionState::ReadingPacket => {
                dbg!("ReadingPacket");
                let req = match this.as_mut().read_packet(cx) {
                    Poll::Ready(Ok(req)) => {
                        dbg!("request OK!!");
                        req
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                };
                if let ControlPacket::DISCONNECT(_) = req.body {
                    // -> DropでClose処理を行う
                    return Poll::Ready(Ok(()));
                }
                let fut = Box::pin((this.service).call(req));
                new_state = Some(ConnectionState::ProcessingService(fut));
            }
            ConnectionState::ProcessingService(mut fut) => {
                let response = match fut.as_mut().poll(cx) {
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
        dbg!("plz, poll again");
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

impl<S, CS, IO> Connection<S, CS, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin,
    CS: Service<
            Request<ControlPacket>,
            Response = connack_response::ConnackResponse,
            Error = connack_response::ConnackError,
        > + Unpin,
    CS::Future: Unpin,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    fn write_connack(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        response: connack_response::ConnackResponse,
    ) -> Poll<Result<(), Box<dyn std::error::Error>>> {
        // [TODO]詰め替えの抑制？
        let mut connack = response.to_connack();
        match connack.build_bytes() {
            Ok(_) => (),
            Err(err) => return Poll::Ready(Err(err.into())),
        }
        let res = response::Response::new(mqtt::ControlPacket::CONNACK(connack));
        dbg!("write packet will", &res);
        return self.write_packet(cx, &res);
    }
    fn read_packet(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Request<mqtt::ControlPacket>, Box<dyn std::error::Error>>> {
        let this = self.project();
        let mut buf = &mut *this.buffer;
        dbg!(&buf);
        match poll_read_buf(this.io, cx, &mut buf) {
            Poll::Pending => {
                if buf.is_empty() {
                    return Poll::Pending;
                }
                // bufferにデータが存在する場合はdecodeを試みる
                match this.decoder.poll_decode(cx, &mut buf) {
                    Poll::Ready(Ok(control_packet)) => {
                        Poll::Ready(Ok(Request::new(control_packet)))
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(Box::new(e))),
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                }
            }
            Poll::Ready(Ok(0)) => {
                return Poll::Ready(Err("Connection closed".into()));
            }
            Poll::Ready(Ok(_n)) => match this.decoder.poll_decode(cx, &mut buf) {
                Poll::Ready(Ok(control_packet)) => Poll::Ready(Ok(Request::new(control_packet))),
                Poll::Ready(Err(e)) => return Poll::Ready(Err(Box::new(e))),
                Poll::Pending => {
                    dbg!("pending2");
                    return Poll::Pending;
                }
            },
            Poll::Ready(Err(e)) => Poll::Ready(Err(Box::new(e))),
        }
    }
    fn write_packet(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        res: &Response,
    ) -> Poll<Result<(), Box<dyn std::error::Error>>> {
        // as_mut() は Pin<&mut Self> のまま再取得する。
        dbg!("Write packet!!!!!!!!");
        let mut this = self.project();

        let encoder = &mut this.encoder;
        let write_buffer = &mut this.write_buffer;
        match encoder.poll_encode(cx, &res.packet, write_buffer) {
            Poll::Ready(Ok(Some(()))) => {
                dbg!("poll encode!");
                while !write_buffer.is_empty() {
                    let n = {
                        let buf = &mut *write_buffer;
                        match Pin::new(&mut this.io).poll_write(cx, buf) {
                            Poll::Ready(Ok(n)) => n,
                            Poll::Ready(Err(e)) => return Poll::Ready(Err(Box::new(e))),
                            Poll::Pending => return Poll::Pending,
                        }
                    };
                    if n == 0 {
                        return Poll::Ready(Err("Connection closed during write".into()));
                    }
                    dbg!(&write_buffer);
                    dbg!("advance!!", n);
                    write_buffer.advance(n);
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
