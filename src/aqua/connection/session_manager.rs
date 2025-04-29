use crate::aqua::connection::response::Response as ConnResponse;
use dashmap::DashMap;
use mqtt_coder::mqtt::ControlPacket;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

#[derive(Clone)]
pub struct Outbound {
    tx: mpsc::Sender<ConnResponse>,
}

impl Outbound {
    /// ControlPacket を送信
    pub fn send(&self, pkt: ControlPacket) -> Result<(), TrySendError<ConnResponse>> {
        self.tx.try_send(ConnResponse::new(pkt))
    }
}

pub trait MetaStore {
    fn new() -> Self
    where
        Self: Sized;
    fn add(&mut self, client_id: String);
    fn remove(&mut self, client_id: String);
    fn list(&mut self);
}

#[derive(Clone)]
pub struct SessionManager<M: MetaStore> {
    inner: Arc<DashMap<String, Outbound>>,
    metastore: M,
}

impl<M: MetaStore> SessionManager<M> {
    pub fn new() -> Self {
        SessionManager {
            inner: Arc::new(DashMap::new()),
            metastore: M::new(),
        }
    }

    pub fn register(&self, client_id: String, outbound: Outbound) {
        self.inner.insert(client_id, outbound);
    }
    pub fn unregister(&self, client_id: String) {
        self.inner.remove(&client_id);
    }
}
