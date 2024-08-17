use pin_project::pin_project;
use std::convert::Infallible;
use std::fmt;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::time::Sleep;
use tower::Layer;
use tower::Service;

#[derive(Debug, Default)]
pub struct TimeoutError(());

impl fmt::Display for TimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad("request timed out")
    }
}

impl std::error::Error for TimeoutError {}

//pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub type BoxError = Infallible;

#[pin_project]
pub struct ResponseFuture<F> {
    #[pin]
    response_future: F,
    #[pin]
    sleep: Sleep,
}

#[derive(Debug, Clone)]
pub struct Timeout<S> {
    inner: S,
    timeout: Duration,
}

impl<S> Timeout<S> {
    pub fn new(inner: S, timeout: Duration) -> Self {
        Timeout { inner, timeout }
    }
}

impl<S, Request> Service<Request> for Timeout<S>
where
    S: Service<Request>,
    S::Error: Into<BoxError>,
{
    type Response = S::Response;
    type Error = BoxError;
    type Future = ResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(Into::into)
    }
    fn call(&mut self, request: Request) -> Self::Future {
        let response_future = self.inner.call(request);
        let sleep = tokio::time::sleep(self.timeout);
        ResponseFuture {
            response_future,
            sleep,
        }
    }
}

use std::{future::Future, pin::Pin};
impl<F, Response, Error> Future for ResponseFuture<F>
where
    F: Future<Output = Result<Response, Error>>,
    Error: Into<BoxError>,
{
    type Output = Result<Response, BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        // First check if the response future is ready.
        match this.response_future.poll(cx) {
            Poll::Ready(result) => {
                // The inner service has a response ready for us or it has
                // failed.
                let result = result.map_err(Into::into);
                return Poll::Ready(result);
            }
            Poll::Pending => {
                // Not quite ready yet...
            }
        }

        // Then check if the sleep is ready. If so the response has taken too
        // long and we have to return an error.
        match this.sleep.poll(cx) {
            Poll::Ready(()) => {
                //let error = Box::new(TimeoutError(()));
                //return Poll::Ready();
            }
            Poll::Pending => {
                // Still some time remaining...
            }
        }

        // If neither future is ready then we are still pending.
        Poll::Pending
    }
}

// A layer for wrapping services in `Timeout`
#[derive(Debug, Clone)]
pub struct TimeoutLayer(Duration);

impl TimeoutLayer {
    pub fn new(delay: Duration) -> Self {
        TimeoutLayer(delay)
    }
}

impl<S> Layer<S> for TimeoutLayer {
    type Service = Timeout<S>;

    fn layer(&self, service: S) -> Timeout<S> {
        Timeout::new(service, self.0)
    }
}
