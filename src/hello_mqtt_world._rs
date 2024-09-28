use std::{
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use tower::Service;

use crate::aqua;

#[derive(Clone)]
pub struct HelloWorld;

impl Service<aqua::Packet> for HelloWorld {
    type Response = aqua::Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: aqua::Packet) -> Self::Future {
        // create the body
        let body: Vec<u8> = "hello, world!\n".as_bytes().to_owned();
        // Create the HTTP response
        let resp = aqua::Response {
            message: "Hello".into(),
        };

        // create a response in a future.
        let fut = async { Ok(resp) };

        // Return the response as an immediate future
        Box::pin(fut)
    }
}
