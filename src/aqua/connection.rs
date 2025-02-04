pub mod request;
pub mod response;

use bytes::{BufMut, BytesMut};
use futures_util::stream::poll_fn;
use mqtt_decoder::decoder;
use tokio::io::AsyncWriteExt
use mqtt_decoder::mqtt;
use mqtt_decoder::mqtt::decoder::decode_fixed_header;
use mqtt_decoder::mqtt::ControlPacket;
use mqtt_decoder::mqtt::MqttError;
use mqtt_decoder::mqtt::MqttPacket;
use request::Request;
use response::Response;
use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::io::poll_read_buf;
use tower::Service;

const BUFFER_CAPACITY: usize = 4096;

pub struct Connection<S, IO> {
    service: S,
    io: IO,
    state: ConnectionState,
    buffer: BytesMut,
    decoder: decoder::Decoder,
}

enum ConnectionState {
    PreConnection,
    ReadingPacket,
    ProcessingService(Request<request::IncomingMqtt>), // Request, Responseはこちらが定義し提供する
    WritingPacket(Response),
    Closed,
}

impl<S, IO> Connection<S, IO>
where
    S: Service<Request<request::IncomingMqtt>, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: 'static,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(service: S, io: IO) -> Self {
        Connection {
            service,
            io,
            state: ConnectionState::PreConnection,
            buffer: BytesMut::with_capacity(BUFFER_CAPACITY), /* [TODO] limit */
            decoder: decoder::Decoder::new(),
        }
    }
}

impl<S, IO> Future for Connection<S, IO>
where
    S: Service<Request<request::IncomingMqtt>, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Output = Result<(), Box<dyn std::error::Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.state {
                ConnectionState::PreConnection => {
                    let req = match self.read_packet(cx) {
                        Poll::Ready(Ok(req)) => {
                            if req != ControlPacket::CONNECT {
                                return Poll::Ready(Err(Box::new(MqttError::Unexpected)));
                            }
                            req
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    };
                    self.state = ConnectionState::ReadingPacket;
                }
                // 要求をReadするフェーズ
                ConnectionState::ReadingPacket => {
                    let req = match self.read_packet(cx) {
                        Poll::Ready(Ok(req)) => req,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                        Poll::Pending => return Poll::Pending,
                    };

                    if req == ControlPacket::DISCONNECT {
                        self.state = ConnectionState::Closed;
                        // -> DropでClose処理を行う
                        return Poll::Ready(Ok(()));
                    }
                    self.state = ConnectionState::ProcessingService(req);
                }
                ConnectionState::ProcessingService(ref req) => {
                    let p = match Pin::new(&mut self.service).poll_ready(cx) {
                        Poll::Ready(Ok(())) => {
                            let fut = self.service.call(*req); /* Clone cost ??? */
                            match Pin::new(&mut fut).poll(cx) {
                                Poll::Ready(Ok(res)) => res,
                                Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                                Poll::Pending => return Poll::Pending,
                            }
                        }
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Pending => return Poll::Pending,
                    };
                    self.state = ConnectionState::WritingPacket(p);
                }
                ConnectionState::WritingPacket(ref res) => match self.write_packet(cx, res) {
                    Poll::Ready(Ok(())) => self.state = ConnectionState::ReadingPacket,
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                },
                ConnectionState::Closed => {
                    return Poll::Ready(Ok(()));
                }
            }
        }
    }
}

impl<S, IO> Connection<S, IO>
where
    S: Service<Request<request::IncomingMqtt>, Response = Response> + Unpin,
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
        &self,
        cx: &mut Context<'_>,
        res: &Response,
    ) -> Poll<Result<(), Box<dyn std::error::Error>>> {
        //let mut buf = BytesMut::new();

        // encodeにおいてもControlPacketに共通のメソッドを用意する
        //res.packet.encode_...(buf, start_pos)
        let buf = match res.packet.encode() {
            Ok(buf) => buf,
            Err(e) => {
                return Poll::Ready(Err(Box::new(e)));
            }
        };
        match Pin::new(&mut self.io).poll_write(cx, &buf) {
            Poll::Ready(Ok(n)) => {
                Poll::Ready(Ok(n))
            },
            Poll::Ready(Err(e)) => Poll::Ready(Err(Box::new(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}
