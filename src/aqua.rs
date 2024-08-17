use std::convert::Infallible;
use std::fmt::Debug;
use std::future::{poll_fn, IntoFuture};
use std::time::Duration;

use futures_util::future::{ready, Ready};
use std::io;
use std::marker::PhantomData;
use std::net::SocketAddr;
use std::task::{Context, Poll};
use tokio::net::{TcpListener, TcpStream};
use tower_service::Service;

pub struct Payload {}

pub struct Packet {
    pub data: String,
}

#[derive(Clone)]
pub struct Response {
    pub message: String,
}

pub fn serve<M, S>(tcp_listener: TcpListener, make_service: M) -> Serve<M, S>
where
    M: for<'a> Service<Packet, Error = Infallible, Response = S>,
    S: Service<Packet, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    Serve {
        tcp_listener,
        make_service,
        _marker: PhantomData,
    }
}

pub struct Serve<M, S> {
    tcp_listener: TcpListener,
    make_service: M,
    _marker: PhantomData<S>,
}

// 1: Debug
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
    M: for<'a> Service<Packet, Error = Infallible, Response = S> + Send + 'static,
    for<'a> <M as Service<Packet>>::Future: Send,
    S: Service<Packet, Response = Response, Error = Infallible> + Clone + Send + 'static,
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
                match tcp_accept(&tcp_listener).await {
                    Some((tcp_stream, remote_addr)) => {
                        println!("Accepted connection from {}", remote_addr);

                        poll_fn(|cx| make_service.poll_ready(cx))
                            .await
                            .unwrap_or_else(|err| match err {});

                        // Call the service with a Packet and get the response
                        match make_service
                            .call(Packet {
                                data: "hello".into(),
                            })
                            .await
                        {
                            Ok(response) => {
                                //println!("Received response: {:?}", response.message);
                            }
                            Err(err) => {
                                eprintln!("Service error: {:?}", err);
                            }
                        }
                    }
                    None => {
                        // Handle error or retry
                        eprintln!("Failed to accept connection, retrying...");
                        //sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        }))
    }
}

type ResponseFuture = Ready<Result<Response, Infallible>>;

impl Service<Packet> for Response {
    type Response = Response;
    type Error = Infallible;
    type Future = ResponseFuture;
    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // サービスがリクエストを受け付ける準備ができているかどうかをチェック
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, req: Packet) -> Self::Future {
        // Packet を受け取って、Response を返すロジック
        let response = Response {
            message: format!("Received: {}", req.data),
        };
        // Ready::Ok(response) を返す
        ready(Ok(response))
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

mod private {
    use std::{
        future::Future,
        io,
        pin::Pin,
        task::{Context, Poll},
    };
    // pub(super) 親モジュールへServeFutureを公開する
    pub struct ServeFuture(pub(super) futures_util::future::BoxFuture<'static, io::Result<()>>);

    impl Future for ServeFuture {
        type Output = io::Result<()>;

        // パフォーマンス向上のために inline化を行う
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
