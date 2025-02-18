pub mod request;
pub mod response;

use bytes::{BufMut, BytesMut};
use futures_util::stream::poll_fn;
use mqtt_coder::decoder;
use mqtt_coder::encoder;
use mqtt_coder::mqtt;
use mqtt_coder::mqtt::decoder::decode_fixed_header;
use mqtt_coder::mqtt::ControlPacket;
use mqtt_coder::mqtt::MqttError;
use mqtt_coder::mqtt::MqttPacket;
use request::Request;
use response::Response;
use std::future::Future;
use std::mem;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncWriteExt;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
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
    decoder: decoder::Decoder,
    encoder: encoder::Encoder,
}

enum ConnectionState<F> {
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
        let this = self.get_mut();
        let state = std::mem::replace(&mut this.state, ConnectionState::Closed);

        let new_state = match state {
            ConnectionState::PreConnection => {
                let _req = match this.read_packet(cx) {
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
                ConnectionState::ReadingPacket
            }
            // 要求をReadするフェーズ
            ConnectionState::ReadingPacket => {
                let req = match Pin::new(&mut *self).read_packet(cx) {
                    Poll::Ready(Ok(req)) => req,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                };
                if let ControlPacket::DISCONNECT(_) = req.body {
                    // -> DropでClose処理を行う
                    return Poll::Ready(Ok(()));
                }
                let fut = self.service.call(req);
                ConnectionState::ProcessingService(fut)
            }
            ConnectionState::ProcessingService(fut) => {
                let response = match Pin::new(&mut fut).poll(cx) {
                    Poll::Ready(Ok(res)) => res, // Response を取得
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                    Poll::Pending => return Poll::Pending,
                };
                ConnectionState::WritingPacket(response)
            }
            ConnectionState::WritingPacket(ref res) => match self.write_packet(cx, res) {
                Poll::Ready(Ok(())) => ConnectionState::ReadingPacket,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            },
            ConnectionState::Closed => {
                return Poll::Ready(Ok(()));
            }
        };
        self.as_mut().state = new_state;
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
        let this = self.get_mut();
        let mut buf = &mut this.buffer;
        match poll_read_buf(Pin::new(&mut this.io), cx, &mut buf) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Ok(0)) => {
                // closed
                // ?
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
    fn write_packet(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        res: &Response,
    ) -> Poll<Result<(), Box<dyn std::error::Error>>> {
        let this = self.get_mut();

        this.encoder.reset();
        loop {
            match this.encoder.poll_encode(cx, &res.packet) {
                Poll::Ready(Some(Ok(chunk))) => {
                    match Pin::new(&mut this.io).poll_write(cx, &chunk) {
                        Poll::Ready(Ok(n)) => {
                            if n == 0 {
                                return Poll::Ready(Err("Connection closed during write".into()));
                            }
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(Box::new(e))),
                        Poll::Pending => return Poll::Pending,
                    }
                }
                Poll::Ready(Some(Err(e))) => return Poll::Ready(Err(e)),
                Poll::Ready(None) => {
                    this.encoder.reset();
                    return Poll::Ready(Ok(()));
                }
                Poll::Pending => return Poll::Pending,
            }
        }
    }
}
