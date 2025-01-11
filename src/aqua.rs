use std::fmt::Debug;
use std::future::IntoFuture;
use std::io;
use std::marker::PhantomData;
use tokio::net::TcpListener;
pub struct Serve<M, S> {
    tcp_listener: TcpListener,
    make_service: M,
    _marker: PhantomData<S>,
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

impl<M, S> IntoFuture for Serve<M, S> {
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
                //[TODO] let tcp_stream = TokioIo::new(tcp_stream);が何をやっているか調べる
                // use hyper_util::rt::{TokioExecutor, TokioIo};これ
                poll_fn(|cx| make_service.poll_ready(cx))
                    .await
                    .unwrap_or_else(|err| match err {});
            }
        }))
    }
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
