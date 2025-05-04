pub mod connack_response;
pub mod request;
pub mod response;
pub mod session_manager;

use bytes::BytesMut;
use mqtt_coder::decoder;
use mqtt_coder::encoder;
use mqtt_coder::mqtt;
use mqtt_coder::mqtt::ControlPacket;
use mqtt_coder::mqtt::MqttError;
use pin_project::pin_project;
use request::Request;
use response::Response;
use std::future::Future;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use tokio::io::AsyncWriteExt;
use tokio::io::{split, AsyncRead, AsyncWrite, ReadHalf, WriteHalf};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;
use tokio::task::JoinHandle;
use tokio_util::io::poll_read_buf;
use tower::Service;
use tracing::{instrument, trace};
use uuid::Uuid;

const BUFFER_CAPACITY: usize = 4096;
const CHANNEL_CAPACITY: usize = 32;
use crate::aqua::connection::session_manager::SessionManager;
use once_cell::sync::Lazy;

pub static SESSION_MANAGER: Lazy<SessionManager> = Lazy::new(|| SessionManager::new());

#[pin_project]
pub struct Connection<S, CS, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Future: Unpin, // `S::Future` を `Unpin` にする
    CS: Service<
            Request<ControlPacket>,
            Response = connack_response::ConnackResponse,
            Error = connack_response::ConnackError,
        > + Unpin,
    CS::Future: Unpin, // `S::Future` を `Unpin` にする
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    client_id: Uuid,
    service: S,
    connect_service: CS,
    #[pin]
    reader: ReadHalf<IO>,
    tx: mpsc::Sender<Response>,
    write_task: JoinHandle<()>,
    state: ConnectionState<S::Future, CS::Future>,
    buffer: BytesMut,
    write_buffer: BytesMut,
    decoder: decoder::Decoder,
    encoder: encoder::Encoder,
}

#[derive(Default)]
enum ConnectionState<F, CF> {
    #[default]
    PreConnection,
    ProcessingConnect(Pin<Box<CF>>),
    ResponseConnect(connack_response::ConnackResponse),
    ReadingPacket,
    ProcessingService(Pin<Box<F>>),
    WritingPacket(Response),
}

impl<S, CS, IO> Connection<S, CS, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin + 'static,
    CS: Service<
            Request<ControlPacket>,
            Response = connack_response::ConnackResponse,
            Error = connack_response::ConnackError,
        > + Unpin,
    CS::Error: std::error::Error + Send + Sync + 'static,
    CS::Future: Unpin + 'static,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    pub fn new(service: S, connect_service: CS, io: IO) -> Self {
        let (reader, writer): (ReadHalf<IO>, WriteHalf<IO>) = split(io);
        let (tx, rx) = mpsc::channel(CHANNEL_CAPACITY);
        let write_task = Self::spawn_writer(writer, rx);
        let client_id = Uuid::new_v4();
        let outbound = session_manager::Outbound::new(tx.clone());

        SESSION_MANAGER.register(client_id, outbound);

        Connection {
            client_id,
            service,
            connect_service,
            reader,
            tx,
            write_task,
            state: ConnectionState::PreConnection,
            buffer: BytesMut::with_capacity(BUFFER_CAPACITY), /* [TODO] limit */
            write_buffer: BytesMut::new(),
            decoder: decoder::Decoder::new(),
            encoder: encoder::Encoder::new(),
        }
    }

    fn spawn_writer<W>(mut writer: W, mut rx: mpsc::Receiver<Response>) -> JoinHandle<()>
    where
        W: AsyncWrite + Unpin + Send + 'static,
    {
        tokio::spawn(async move {
            let mut encoder = encoder::Encoder::new();
            let mut write_buffer = BytesMut::new();
            while let Some(res) = rx.recv().await {
                let packet = res.packet;
                match encoder.encode_all(&packet, &mut write_buffer) {
                    Ok(()) => {}
                    Err(e) => {
                        eprintln!("Encode error: {:?}", e);
                        return;
                    }
                }

                while !write_buffer.is_empty() {
                    match writer.write_buf(&mut write_buffer).await {
                        Ok(0) => {
                            eprintln!("Connection closed during write");
                            return;
                        }
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("Write error: {:?}", e);
                            return;
                        }
                    }
                }
            }
        })
    }
}

impl<S, CS, IO> Future for Connection<S, CS, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin,
    CS: Service<
            Request<ControlPacket>,
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
        let mut new_state = Some(ConnectionState::PreConnection);
        match std::mem::take(&mut this.state) {
            // ここはConnectパケットを受け経ったかどうかのみ：接続管理
            ConnectionState::PreConnection => {
                let req = match this.as_mut().read_packet(cx) {
                    Poll::Ready(Ok(req)) => {
                        if let ControlPacket::CONNECT(ref packet) = req.body {
                            // here ??
                            this.decoder.set_protocol_version(Some(packet.protocol_ver));
                            req
                        } else {
                            trace!("Unexpected because, this is not connect packet {:?}", req);
                            return Poll::Ready(Err(Box::new(MqttError::Unexpected)));
                        }
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending, /* Pendingで終わるため、new_stateが変わらない */
                };
                trace!("connection reading packet {:?}", req);
                let fut = Box::pin((this.connect_service).call(req));
                new_state = Some(ConnectionState::ProcessingConnect(fut));
            }
            ConnectionState::ProcessingConnect(mut fut) => {
                let connect_service_result = match fut.as_mut().poll(cx) {
                    Poll::Ready(Ok(res)) => res, // Response を取得
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                    Poll::Pending => return Poll::Pending,
                };

                new_state = Some(ConnectionState::ResponseConnect(connect_service_result));
            }
            ConnectionState::ResponseConnect(res) => {
                trace!("response connect");
                let connack = res.to_connack();
                let res = response::Response::new(mqtt::ControlPacket::CONNACK(connack));

                // Self
                match this.tx.try_send(res) {
                    Ok(()) => {}
                    Err(TrySendError::Full(resp)) => {
                        eprintln!("Channel Full droping response: {:?}", resp);
                    }
                    Err(TrySendError::Closed(_)) => {
                        return Poll::Ready(Err("Channel closed".into()));
                    }
                }
                new_state = Some(ConnectionState::ReadingPacket);
            }
            // 要求をReadするフェーズ
            ConnectionState::ReadingPacket => {
                trace!("reading packet");
                let req = match this.as_mut().read_packet(cx) {
                    Poll::Ready(Ok(req)) => {
                        println!("read packet done!!");
                        req
                    }
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                    Poll::Pending => return Poll::Pending,
                };
                trace!("done!!!!!");
                if let ControlPacket::DISCONNECT(_) = req.body {
                    // -> DropでClose処理を行う
                    return Poll::Ready(Ok(()));
                }
                let fut = Box::pin((this.service).call(req));
                new_state = Some(ConnectionState::ProcessingService(fut));
            }
            ConnectionState::ProcessingService(mut fut) => {
                trace!("processing service");

                let response = match fut.as_mut().poll(cx) {
                    Poll::Ready(Ok(res)) => res, // Response を取得
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                    Poll::Pending => return Poll::Pending,
                };
                new_state = Some(ConnectionState::WritingPacket(response));
            }
            ConnectionState::WritingPacket(res) => {
                trace!("writing packet");

                match this.tx.try_send(response::Response::new(res.packet)) {
                    Ok(()) => {}
                    Err(TrySendError::Full(resp)) => {
                        eprintln!("Channel Full droping response: {:?}", resp);
                    }
                    Err(TrySendError::Closed(_)) => {
                        return Poll::Ready(Err("Channel closed".into()));
                    }
                }
                new_state = Some(ConnectionState::ReadingPacket);
            }
        }
        if let Some(state) = new_state {
            this.state = state;
        }
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

impl<S, CS, IO> Connection<S, CS, IO>
where
    S: Service<Request<ControlPacket>, Response = Response> + Unpin,
    S::Error: std::error::Error + Send + Sync + 'static,
    S::Future: Unpin,
    CS: Service<
            Request<ControlPacket>,
            Response = connack_response::ConnackResponse,
            Error = connack_response::ConnackError,
        > + Unpin,
    CS::Future: Unpin,
    IO: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    fn read_packet(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<Request<mqtt::ControlPacket>, Box<dyn std::error::Error>>> {
        let this = self.project();
        let mut buf = &mut *this.buffer;
        let read_buf_res = match poll_read_buf(this.reader, cx, &mut buf) {
            Poll::Pending => {
                println!("poll_read_buf → Pending, buf.len = {}", buf.len());
                if buf.is_empty() {
                    return Poll::Pending;
                }
                // bufferにデータが存在する場合はdecodeを試みる
                let packet = this.decoder.poll_decode(cx, &mut buf);
                match packet {
                    Poll::Ready(Ok(control_packet)) => {
                        trace!("ready???");
                        let res = Poll::Ready(Ok(Request::new(control_packet)));
                        trace!("res {:?}", res);

                        res
                    }
                    Poll::Ready(Err(e)) => {
                        return Poll::Ready(Err(Box::new(e) as Box<dyn std::error::Error>))
                    }
                    Poll::Pending => {
                        return Poll::Pending;
                    }
                }
            }
            Poll::Ready(Ok(0)) => {
                return Poll::Ready(Err("Connection closed".into()));
            }
            Poll::Ready(Ok(_n)) => match this.decoder.poll_decode(cx, &mut buf) {
                Poll::Ready(Ok(control_packet)) => Poll::Ready(Ok(Request::new(control_packet))),
                Poll::Ready(Err(e)) => {
                    return Poll::Ready(Err(Box::new(e) as Box<dyn std::error::Error>))
                }
                Poll::Pending => {
                    return Poll::Pending;
                }
            },
            Poll::Ready(Err(e)) => Poll::Ready(Err(Box::new(e) as Box<dyn std::error::Error>)),
        };
        trace!("readbuf return {:?}", read_buf_res);
        return read_buf_res;
    }
}
