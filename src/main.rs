mod aqua;
mod hello_mqtt_world;
mod hello_world;
mod mqtt;
mod org_axus;
mod timeout;
use std::time::Duration;

use axum::{response::Html, routing::get, Router};
use tower::ServiceBuilder;

async fn handler() -> &'static str {
    "Hello world"
}

#[tokio::main]
async fn main() {
    //let app = Router::new().route("/", get(handler));
    // .layer(ServiceBuilder::new().layer(timeout::TimeoutLayer::new(Duration::from_secs(1))));

    //let app = ServiceBuilder::new().layer(timeout::TimeoutLayer::new(Duration::from_secs(1))).;
    let svc = hello_mqtt_world::HelloWorld;
    //et res = svc.call(req).await.unwrap();
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001").await.unwrap();
    //axum::serve(listener, app).await.unwrap();
    //let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    //org_axus::serve(listener, app).await.unwrap();
    //axum::serve(listener, app).await.unwrap();
    aqua::serve(listener, svc).await.unwrap();
}

async fn root() -> &'static str {
    "Hello, World!"
}
