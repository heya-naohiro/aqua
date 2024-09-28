use std::convert::Infallible;
use std::fmt::Debug;
use std::future::{poll_fn, IntoFuture};
use std::time::Duration;

use crate::mqtt::{self, MqttPacket};
use futures_util::future::{ready, Ready};
use hyper_util::rt::TokioExecutor;
use std::io;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::task::{Context, Poll};
use tokio::net::{TcpListener, TcpStream};
use tower_service::Service;

pub struct Serve<M, S> {
    tcp_listener: TcpListener,
    make_service: M,
    _marker: PhantomData<S>,
}

/* tentative */
#[derive(Clone)]
pub struct Response {
    pub message: String,
}

impl<M, S> IntoFuture for Serve<M, S>
where
    M: for<'a> Service<mqtt::MqttPacket, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<mqtt::MqttPacket>>::Future: Send,
    S: Service<mqtt::MqttPacket, Response = Response, Error = Infallible> + Clone + Send + 'static,
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
                /*match tcp_accept(&tcp_listener).await {
                    Some((tcp_stream, remote_addr)) => {
                        poll_fn(|cx| make_service.poll_ready(cx))
                            .await
                            .unwrap_or_else(|err| match err {});

                        let tower_service = make_service
                            .call(aqua::IncomingStream {
                                tcp_stream: &tcp_stream,
                                remote_addr,
                            })
                            .await
                            .unwrap_or_else(|err| match err {});

                        let aqua_service = TowerToAquaService {
                            service: tower_service,
                        };

                        tokio::spawn(async move {
                            match Builder::new(TokioExecutor::new())
                                .serve_connection(socket, aqua_service)
                                .await
                            {
                                Ok(()) => {}
                                Err(_err) => {}
                            }
                        })
                    }

                    None => {
                        eprintln!("Failed to accept connection, retrying...");
                    }
                }
                */
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
