use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::{AsyncRead, AsyncWrite};
use tower::Service;
mod request;
mod response;

pub struct Connection<S, IO> {
    service: S,
    io: IO,
    state: ConnectionState,
}

enum ConnectionState {
    PreConnection,
    ReadingPacket,
    ProcessingService(Request<MqttPacket>), // Request, Responseはこちらが定義し提供する
    WritingPacket(Response<MqttPacket>),
    Closed,
}

impl<S, IO> Connection<S, IO>
where
    S: Service<Request<MqttPacket>, Response = Response<MqttPacket>> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(service: S, io: IO) -> Self {
        Connection {
            service,
            io,
            state: ConnectionState::PreConnection,
        }
    }
}

impl<S, IO> Future for Connection<S, IO>
where
    S: Service<Request<MqttPacket>, Response = Response<MqttPacket>> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Output = Result<(), std::error::Error>;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        loop {
            match self.state {
                ConnectionState::PreConnection => {
                    let req = match self.read_packet(cx) {
                        Poll::Ready(Ok(req)) => {
                            if req != MqttPacket::Connect {
                                return Poll::Ready(Err(e));
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

                    if req == MqttPacket::Close {
                        self.state = ConnectionState::Closed;
                        // -> DropでClose処理を行う
                        return Poll::Ready(Ok(()));
                    }
                    self.state = ConnectionState::ProcessingPacket(req);
                }
                ConnectionState::ProcessingPacket(ref req) => {
                    let p = match Pin::new(&mut self.service).poll_ready(cx) {
                        Poll::Ready(Ok(())) => {
                            let fut = self.service.call(req.clone()); /* Clone cost ??? */
                            match Pin::New(&mut fut).poll(cx) {
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
                ConnectionState::Writing(ref res) => match self.write_packet(cx, res) {
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
