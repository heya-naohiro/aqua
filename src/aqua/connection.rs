pub mod connack_response;
pub mod request;
pub mod response;
pub mod retain_store;
pub mod session_manager;

use bytes::BytesMut;
use mqtt_coder::decoder;
use mqtt_coder::encoder;
use mqtt_coder::mqtt;
use mqtt_coder::mqtt::ControlPacket;
use mqtt_coder::mqtt::MqttError;
use pin_project::pin_project;
use response::Response;
use std::collections::VecDeque;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncWriteExt;
use tokio::io::{split, AsyncRead, AsyncWrite, ReadHalf, WriteHalf};
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tokio_util::io::poll_read_buf;
use tower::Service;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;
use uuid::Uuid;
//const BUFFER_CAPACITY: usize = 4096;
const CHANNEL_CAPACITY: usize = 32;
use crate::aqua::connection::session_manager::SessionManager;
use once_cell::sync::Lazy;

pub static SESSION_MANAGER: Lazy<SessionManager> = Lazy::new(|| SessionManager::new());

#[pin_project]
pub struct Connection<S, CS, IO>
where
    S: Service<WriteRequest, Response = Response> + Unpin,
    S::Future: Unpin, // `S::Future` を `Unpin` にする
    CS: Service<
            WriteRequest,
            Response = connack_response::ConnackResponse,
            Error = connack_response::ConnackError,
        > + Unpin,
    CS::Future: Unpin, // `S::Future` を `Unpin` にする
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    client_id: Uuid,
    mqtt_id: Option<String>,
    service: S,
    connect_service: CS,
    #[pin]
    reader: ReadHalf<IO>,
    tx: mpsc::Sender<WriteRequest>,
    write_task: JoinHandle<()>,
    state: ConnectionState<S::Future, CS::Future>,
    write_buffer: BytesMut,
    decoder: decoder::Decoder,
    encoder: encoder::Encoder,
    protocol_version: Option<mqtt::ProtocolVersion>,
}

#[derive(Default)]
enum ConnectionState<F, CF> {
    #[default]
    PreConnection, // index 0
    ProcessingConnect(Pin<Box<CF>>),                    // index 1
    ResponseConnect(connack_response::ConnackResponse), // index 2
    SendingFollowUps(VecDeque<ControlPacket>),
    ReadingPacket,                  // index 3
    ProcessingService(Pin<Box<F>>), // index 4
    WritingPacket(Response),        // index 5
}

#[derive(Clone, Debug)]
pub struct WriteRequest {
    pub packet: ControlPacket,
    pub mqtt_id: String,
}

impl<S, CS, IO> Connection<S, CS, IO>
where
    S: Service<WriteRequest, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin + 'static,
    CS: Service<
            WriteRequest,
            Response = connack_response::ConnackResponse,
            Error = connack_response::ConnackError,
        > + Unpin,
    CS::Error: std::error::Error + Send + Sync + 'static,
    CS::Future: Unpin + 'static,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(service: S, connect_service: CS, io: IO, client_id: Uuid) -> Self {
        let (reader, writer): (ReadHalf<IO>, WriteHalf<IO>) = split(io);
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let write_task = Self::spawn_writer(writer, rx);

        let outbound = session_manager::Outbound::new(tx.clone());
        SESSION_MANAGER.register_client_id(client_id, outbound);
        debug!(
            "=== Connection::new() called with client_id {:?}",
            client_id
        );
        Connection {
            client_id,
            service,
            connect_service,
            reader,
            tx,
            write_task,
            state: ConnectionState::PreConnection,
            write_buffer: BytesMut::new(),
            decoder: decoder::Decoder::new(),
            encoder: encoder::Encoder::new(),
            protocol_version: None,
            mqtt_id: None,
        }
    }

    fn spawn_writer<W>(mut writer: W, mut rx: mpsc::Receiver<WriteRequest>) -> JoinHandle<()>
    where
        W: AsyncWrite + Unpin + Send + 'static,
    {
        tokio::spawn(async move {
            let mut encoder = encoder::Encoder::new();
            let mut write_buffer = BytesMut::new();
            while let Some(write_req) = rx.recv().await {
                info!("write_req {:?}", write_req);

                match encoder.encode_all(&write_req.packet, &mut write_buffer) {
                    Ok(()) => {}
                    Err(e) => {
                        error!("Encode error: {:?} {:?}", &write_req.packet, e);
                        return;
                    }
                }

                while !write_buffer.is_empty() {
                    match writer.write_buf(&mut write_buffer).await {
                        Ok(0) => {
                            error!("Connection closed during write");
                            return;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            error!("write_error");
                            if write_req.mqtt_id != "" {
                                info!(
                                    "add to queue, {:?} {:?}",
                                    write_req.mqtt_id, write_req.packet
                                );
                            }
                            error!("Write error: {:?}", e);
                            return;
                        }
                    }
                }
            }
            if let Err(e) = writer.shutdown().await {
                eprintln!("Writer shutdown failed: {:?}", e);
            }
        })
    }
}

impl<S, CS, IO> Future for Connection<S, CS, IO>
where
    S: Service<WriteRequest, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin,
    CS: Service<
            WriteRequest,
            Response = connack_response::ConnackResponse,
            Error = connack_response::ConnackError,
        > + Unpin,
    CS::Error: std::error::Error + Send + Sync + 'static,
    CS::Future: Unpin,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    type Output = Result<(), Box<dyn std::error::Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.as_mut();
        let new_state;
        // [注意]早期リターンするとデフォルトに戻る
        match std::mem::take(&mut this.state) {
            // ここはConnectパケットを受け経ったかどうかのみ：接続管理
            ConnectionState::PreConnection => {
                trace!("state: ConnectionState::PreConnection");
                let req = match this.as_mut().read_packet(cx) {
                    Poll::Ready(Ok(req)) => {
                        if let ControlPacket::CONNECT(ref packet) = req.packet {
                            // here ??
                            // approve mqtt_id implicity
                            this.mqtt_id = Some(packet.client_id.clone().into_inner());
                            this.protocol_version = Some(packet.protocol_ver);
                            this.decoder.set_protocol_version(Some(packet.protocol_ver));
                            req
                        } else {
                            trace!("Unexpected because, this is not connect packet {:?}", req);
                            return Poll::Ready(Err(Box::new(MqttError::Unexpected)));
                        }
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => {
                        this.state = ConnectionState::PreConnection;

                        return Poll::Pending;
                    }
                };
                trace!("connection reading packet {:?}", req);
                let fut = Box::pin((this.connect_service).call(req));
                trace!("next state: Some(ConnectionState::ProcessingConnect(fut)");
                new_state = Some(ConnectionState::ProcessingConnect(fut));
            }
            ConnectionState::ProcessingConnect(mut fut) => {
                trace!("state: ConnectionState::ProcessingConnect(mut fut)");
                let connect_service_result = match fut.as_mut().poll(cx) {
                    Poll::Ready(Ok(res)) => res, // Response を取得
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                    Poll::Pending => {
                        this.state = ConnectionState::ProcessingConnect(fut);

                        return Poll::Pending;
                    }
                };

                new_state = Some(ConnectionState::ResponseConnect(connect_service_result));
                trace!("state: new_state = Some(ConnectionState::ResponseConnect(connect_service_result));");
            }
            ConnectionState::ResponseConnect(res) => {
                trace!("state: ConnectionState::ResponseConnect(res)");
                let (connack, followup_queue) = res.into_connack_and_queue();
                let respkt = mqtt::ControlPacket::CONNACK(connack);

                // Self
                let mqtt_id = this.mqtt_id.clone().unwrap();
                match this.tx.try_send(WriteRequest {
                    packet: respkt,
                    mqtt_id: mqtt_id,
                }) {
                    Ok(()) => {}
                    Err(TrySendError::Full(resp)) => {
                        eprintln!("Channel Full droping response: {:?}", resp);
                    }
                    Err(TrySendError::Closed(_)) => {
                        return Poll::Ready(Err("Channel closed".into()));
                    }
                }
                if followup_queue.len() > 0 {
                    new_state = Some(ConnectionState::SendingFollowUps(followup_queue));
                    trace!("state: new_state = Some(ConnectionState::SendingFollowUps(followup_queue));");
                } else {
                    new_state = Some(ConnectionState::ReadingPacket);
                    trace!("state: new_state = Some(ConnectionState::ReadingPacket);");
                }
            }
            ConnectionState::SendingFollowUps(mut packets) => {
                if let Some(packet) = packets.pop_front() {
                    let mqtt_id = this.mqtt_id.clone().unwrap();
                    match this.tx.try_send(WriteRequest { packet, mqtt_id }) {
                        Ok(()) => info!("sent follow-up packet"),
                        Err(e) => {
                            error!("failed to send follow-up: {:?}", e);
                            return Poll::Ready(Err("Channel Error".into()));
                        }
                    }
                    if !packets.is_empty() {
                        new_state = Some(ConnectionState::SendingFollowUps(packets));
                    } else {
                        new_state = Some(ConnectionState::ReadingPacket);
                    }
                } else {
                    new_state = Some(ConnectionState::ReadingPacket);
                }
            }
            // 要求をReadするフェーズ
            ConnectionState::ReadingPacket => {
                trace!("state: ConnectionState::ReadingPacket");
                let req = match this.as_mut().read_packet(cx) {
                    Poll::Ready(Ok(req)) => req,
                    Poll::Ready(Err(e)) => {
                        error!("Error {:?}", e);
                        return Poll::Ready(Err(e));
                    }
                    Poll::Pending => {
                        this.state = ConnectionState::ReadingPacket;

                        return Poll::Pending;
                    }
                };
                let fut = Box::pin((this.service).call(req));
                trace!("state: new_state = Some(ConnectionState::ProcessingService(fut));");
                new_state = Some(ConnectionState::ProcessingService(fut));
            }
            ConnectionState::ProcessingService(mut fut) => {
                trace!("state: ConnectionState::ProcessingService(mut fut)");

                let response = match fut.as_mut().poll(cx) {
                    Poll::Ready(Ok(res)) => res, // Response を取得
                    Poll::Ready(Err(e)) => {
                        return Poll::Ready(Err(e.into()));
                    }
                    Poll::Pending => {
                        this.state = ConnectionState::ProcessingService(fut);

                        return Poll::Pending;
                    }
                };
                new_state = Some(ConnectionState::WritingPacket(response));
                trace!("state: new_state = Some(ConnectionState::WritingPacket(response));");
            }
            ConnectionState::WritingPacket(res) => {
                trace!("state: ConnectionState::WritingPacket(res) {:?}", &res);
                for packet in res.packets {
                    match packet {
                        ControlPacket::DISCONNECT(_) => {
                            return Poll::Ready(Ok(()));
                        }
                        ControlPacket::NOOPERATION => {}
                        _ => {
                            let mqtt_id = this.mqtt_id.clone().unwrap();
                            match this.tx.try_send(WriteRequest {
                                packet: packet,
                                mqtt_id: mqtt_id,
                            }) {
                                Ok(()) => {
                                    trace!("success send");
                                }
                                Err(TrySendError::Full(resp)) => {
                                    eprintln!("Channel Full droping response: {:?}", resp);
                                }
                                Err(TrySendError::Closed(_)) => {
                                    return Poll::Ready(Err("Channel closed".into()));
                                }
                            }
                            trace!("state: new_state = Some(ConnectionState::ReadingPacket);");
                        }
                    }
                }

                new_state = Some(ConnectionState::ReadingPacket);
            }
        }
        if let Some(state) = new_state {
            this.state = state;
            trace!("state changed: {:?}", std::mem::discriminant(&this.state));
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

impl<S, CS, IO> Connection<S, CS, IO>
where
    S: Service<WriteRequest, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin,
    CS: Service<
            WriteRequest,
            Response = connack_response::ConnackResponse,
            Error = connack_response::ConnackError,
        > + Unpin,
    CS::Future: Unpin,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    fn read_packet(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<WriteRequest, Box<dyn std::error::Error>>> {
        let this = self.project();

        match poll_read_buf(this.reader, cx, &mut this.decoder.buf) {
            Poll::Ready(Ok(0)) => {
                info!("Close Channel...");
                drop(this.tx.clone()); // close channel because this cannot be sent.
                return Poll::Ready(Err("Connection closed because poll_Read_buf is zero".into()));
            }
            Poll::Ready(Ok(_n)) => {}
            Poll::Ready(Err(e)) => {
                return Poll::Ready(Err(Box::new(e)));
            }
            Poll::Pending => {
                // fallthrough
            }
        }
        match this.decoder.poll_decode(cx) {
            Poll::Ready(Ok(p)) => {
                info!("decode success {:?}", p);
                Poll::Ready(Ok(WriteRequest {
                    packet: p,
                    mqtt_id: "".to_string(),
                }))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(Box::new(e))),
            Poll::Pending => {
                return Poll::Pending;
            }
        }
    }
}
