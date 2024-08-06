use std::future::IntoFuture;
use std::io;

pub fn add(left: usize, right: usize) -> usize {
    left + right
}

pub fn serve() -> Serve {
    Serve {}
}

impl IntoFuture for Serve {
    type Output = io::Result<()>;
    type IntoFuture = private::ServeFuture;

    fn into_future(self) -> Self::IntoFuture {
        private::ServeFuture(Box::pin(async move {
            println!("Hello, into_future!");
            Ok(())
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
}

pub struct Serve {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
