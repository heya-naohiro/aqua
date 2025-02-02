use futures_util::FutureExt;
// https://github.com/tokio-rs/axum/blob/e09cc593655de82d01971b55130a83842ac46684/axum/src/serve/mod.rs#L351
// 参考
// Listener関連
// https://github.com/tokio-rs/axum/blob/main/axum/src/serve/listener.rs#L9
use mqtt_decoder::mqtt::{ControlPacket, MqttPacket};
use std::convert::Infallible;
use std::fmt::Debug;
use std::future::{poll_fn, Future, IntoFuture};
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::time::Duration;
use std::{io, result};
use tokio::net::{TcpListener, TcpStream};
use tower::ServiceExt;
use tower_service::Service;
use tracing::trace;
mod connection;
use futures_util::pin_mut;

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

pub fn serve<M, S>(tcp_listener: TcpListener, make_service: M) -> Serve<M, S> {
    Serve {
        tcp_listener,
        make_service,
        _marker: PhantomData,
    }
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
    M: for<'a> Service<connection::request::IncomingStream, Error = Infallible, Response = S>,
    S: Service<
            connection::request::Request<connection::request::IncomingStream>,
            Response = connection::response::Response,
            Error = Infallible,
        > + ServiceExt<connection::request::Request<connection::request::IncomingStream>>
        + Clone
        + Send
        + 'static,
    S::Future: Send,
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

                let tower_service = make_service
                    .call(connection::request::IncomingStream {
                        tcp_stream: tcp_stream,
                        addr: remote_addr,
                    })
                    .await
                    .unwrap_or_else(|err| match err {});

                pin_mut!(tower_service);
                tokio::spawn(async move {
                    let conn = connection::Connection::new(tower_service, tcp_stream);
                    pin_mut!(conn);

                    // ここまで前処理
                    // ここが実働部
                    loop {
                        tokio::select! {
                            result = conn.as_mut() => {
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

    impl std::fmt::Debug for ServeFuture {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("ServeFuture").finish_non_exhaustive()
        }
    }
}
