// https://github.com/tokio-rs/axum/blob/e09cc593655de82d01971b55130a83842ac46684/axum/src/serve/mod.rs#L351
// 参考
// Listener関連
// https://github.com/tokio-rs/axum/blob/main/axum/src/serve/listener.rs#L9
use mqtt_coder::mqtt::ControlPacket;
use std::convert::Infallible;
use std::fmt::Debug;
use std::future::{poll_fn, IntoFuture};
use std::io;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tower_service::Service;
use tracing::trace;

pub(crate) mod connection;

pub struct Serve<M, S> {
    tcp_listener: TcpListener,
    make_service: M,
    _marker: PhantomData<S>,
}

pub struct MqttPacketBody {
    pub mqttpacket: ControlPacket,
}

impl MqttPacketBody {
    pub fn new<B>(_body: B) -> Self {
        Self {
            mqttpacket: ControlPacket::UNDEFINED,
        }
    }
}

pub async fn serve<M, S>(tcp_listener: TcpListener, make_service: M) -> io::Result<()>
where
    M: for<'a> Service<connection::request::IncomingStream, Error = Infallible, Response = S>
        + Send
        + 'static,
    for<'a> <M as Service<connection::request::IncomingStream>>::Future: Send,
    S: Service<
            connection::request::Request<ControlPacket>,
            Response = connection::response::Response,
        > + Unpin
        + Clone
        + Send
        + 'static,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Send + Unpin,
{
    // Serve の IntoFuture 実装を利用して Future を返す
    Box::pin(
        Serve {
            tcp_listener,
            make_service,
            _marker: PhantomData,
        }
        .into_future(),
    )
    .await
}

impl<M, S> Debug for Serve<M, S>
where
    M: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self {
            tcp_listener,
            make_service,
            _marker: _,
        } = self;

        f.debug_struct("Serve")
            .field("tcp_listener", tcp_listener)
            .field("make_service", make_service)
            .finish()
    }
}

impl<M, S> IntoFuture for Serve<M, S>
where
    M: for<'a> Service<connection::request::IncomingStream, Error = Infallible, Response = S>
        + Send
        + 'static,
    for<'a> <M as Service<connection::request::IncomingStream>>::Future: Send,
    S: Service<
            connection::request::Request<ControlPacket>,
            Response = connection::response::Response,
        > + Unpin
        + Clone
        + Send
        + 'static,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Send + Unpin,
{
    type Output = io::Result<()>;
    type IntoFuture = private::ServeFuture;

    fn into_future(self) -> Self::IntoFuture {
        private::ServeFuture(Box::pin(async move {
            let Self {
                tcp_listener,
                mut make_service,
                _marker: _,
            } = self;

            loop {
                let (tcp_stream, remote_addr) = match tcp_accept(&tcp_listener).await {
                    Some(conn) => conn,
                    None => continue,
                };
                poll_fn(|cx| make_service.poll_ready(cx))
                    .await
                    .unwrap_or_else(|err| match err {});
                let arc_tcpstream = Arc::new(tcp_stream);

                let tower_service = make_service
                    .call(connection::request::IncomingStream {
                        tcp_stream: arc_tcpstream.clone(),
                        addr: remote_addr,
                    })
                    .await
                    .unwrap_or_else(|err| match err {});

                tokio::spawn(async move {
                    let conn = connection::Connection::new(
                        tower_service,
                        Arc::<tokio::net::TcpStream>::try_unwrap(arc_tcpstream).unwrap(),
                    );

                    // ここまで前処理
                    // ここが実働部
                    loop {
                        tokio::select! {
                            result = conn => {
                                if let Err(err) = result {
                                    trace!("failed to serve connection: {:?}", err);
                                }
                                break;
                            }
                        }
                        /*
                        _ = &mut signal_closed => {
                            trace!("signal received in task, starting graceful shutdown");
                            conn.as_mut().graceful_shutdown();
                        }
                        */
                    }
                });
            }
        }))
    }
}

async fn tcp_accept(listener: &TcpListener) -> Option<(TcpStream, SocketAddr)> {
    match listener.accept().await {
        Ok(conn) => Some(conn),
        Err(e) => {
            if is_connection_error(&e) {
                return None;
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
            None
        }
    }
}

fn is_connection_error(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::ConnectionRefused
            | io::ErrorKind::ConnectionAborted
            | io::ErrorKind::ConnectionReset
    )
}

mod private {
    use std::{
        future::Future,
        io,
        pin::Pin,
        task::{Context, Poll},
    };
    pub struct ServeFuture(pub(super) futures_util::future::BoxFuture<'static, io::Result<()>>);

    impl Future for ServeFuture {
        type Output = io::Result<()>;

        #[inline]
        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.0.as_mut().poll(cx)
        }
    }
    impl Unpin for ServeFuture {}

    impl std::fmt::Debug for ServeFuture {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("ServeFuture").finish_non_exhaustive()
        }
    }
}
