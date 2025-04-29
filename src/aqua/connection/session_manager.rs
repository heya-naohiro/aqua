use crate::aqua::connection::response::Response;
use dashmap::DashMap;
use mqtt_coder::mqtt::ControlPacket;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use uuid::Uuid;

#[derive(Clone)]
pub struct Outbound {
    tx: mpsc::Sender<Response>,
}

impl Outbound {
    pub fn new(tx: mpsc::Sender<Response>) -> Self {
        Self { tx }
    }

    /// ControlPacket を送信
    pub fn send(&self, pkt: ControlPacket) -> Result<(), TrySendError<Response>> {
        self.tx.try_send(Response::new(pkt))
    }
}

#[derive(Clone)]
pub struct SessionManager {
    /* client id : Outbound */
    inner: Arc<DashMap<Uuid, Outbound>>,
}

impl SessionManager {
    pub fn new() -> Self {
        SessionManager {
            inner: Arc::new(DashMap::new()),
        }
    }

    pub fn register(&self, client_id: Uuid, outbound: Outbound) {
        self.inner.insert(client_id, outbound);
    }
    pub fn unregister(&self, client_id: Uuid) {
        self.inner.remove(&client_id);
    }
}
