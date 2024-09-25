use crate::mqtt;
use anyhow::{anyhow, Context, Result};
use bytes::{Buf, BytesMut};
use hyper::body::Frame;
use std::convert::Infallible;
use std::io::Cursor;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufWriter, ReadBuf};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tower::Service;

use crate::mqtt::MqttPacket;
pub struct Server {
    addr: str,
}

enum Operation {
    ReturnPacket,
    NOP,
}

pub struct Response {
    command: Operation,
    packet: mqtt::MqttPacket,
}

pub struct MqttConnection {
    id: str,
}

pub struct Connection {
    stream: TcpStream,
    buffer: BytesMut,
    mqttconnection: Option<MqttConnection>,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Connection {
        Connection {
            stream,
            // 4KB のキャパシティをもつバッファを確保する
            buffer: BytesMut::with_capacity(4096),
            mqttconnection: None,
        }
    }
    pub async fn read_bytes_until_satisfied(&mut self, num_bytes: usize) -> Result<BytesMut> {
        while self.buffer.len() < num_bytes {
            let n = self
                .stream
                .read(&mut self.buffer[self.buffer.len()..])
                .await?;
            if n == 0 {
                return anyhow!("Unexpected EOF");
            }
        }
    }

    /*
    | MQTT Control Packet type, | Flags specific to each MQTT Control Packet type |
    | Remaining Length(1-3)                                                       |
     */
    pub async fn read_mqtt_frames(&mut self) -> Result<MqttIncoming> {
        self.read_bytes_until_satisfied(2);
        let com = self.read_fixed_header().await?;
        // ramain lengthが存在するか確認する
        // Publishでデータのサイズを考慮する（大きいデータは今バッファーになくてもよいことにする）
        match com.mqttpacket.control_packet {
            mqtt::ControlPacket::CONNECT(connect) => {
                // wait all remaining length, including payload
                self.read_bytes_until_satisfied(com.mqttpacket.remaining_length);
                let proceed_varheader = connect.parse_variable_header(&self.buffer)?;
                com.state = FrameState::ReadVariableHeader;
                self.buffer.advance(proceed_varheader);
                let proceed_payload = connect.parse_payload(&self.buffer)?;
                com.state = FrameState::ReadPayload;
                self.buffer.advance(proceed_payload);

                // length validate
                if proceed_varheader + proceed_payload == com.mqttpacket.remaining_length {
                    return anyhow!("remain length is not consistent remaining length is {}, but read length is {}", proceed_varheader + proceed_payload, com.mqttpacket.remaining_length);
                }
                return Ok(com);
            }
            mqtt::ControlPacket::DISCONNECT(disconnect) => {}
        }
    }

    pub async fn read_fixed_header(&mut self) -> Result<MqttIncoming> {
        let minimum_fixed_header = 2;
        for n in 1..=3 {
            match mqtt::mqtt::parse_fixed_header(&mut self.buffer) {
                Ok((packet, proceed)) => {
                    self.buffer.advance(proceed);
                    Ok(MqttIncoming {
                        mqttpacket: packet,
                        state: FrameState::ReadFixedHeader,
                    })
                }
                mqtt::MqttError::InsufficientBytes => {
                    self.read_bytes_until_satisfied(minimum_fixed_header + n);
                }
                Err(e) => return anyhow::anyhow!("Fixed Header Parse Error {}", e),
            }
        }
        return anyhow::anyhow!("Fixed Header Invalid");
    }

    pub async fn write_packet(&mut self, packet: &impl mqtt::WriteBytes) {
        match self.stream.write_buf(packet.to_byte()).await {
            Ok()
        }
    }
}

enum FrameState {
    ReadFixedHeader,
    ReadVariableHeader,
    ReadPayload,
}

pub struct MqttIncoming {
    pub mqttpacket: mqtt::MqttPacket,
    state: FrameState,
}

impl Server {
    async fn run<T>(self, mut connection_handler: T) -> Result<(), anyhow::Error>
    where
        T: tower::Service<MqttIncoming, Response = Response, Error = Infallible>,
    {
        let listener = TcpListener::bind(self.addr).await?;
        loop {
            let (mut stream, remote_addr) = listener.accept().await?;
            tokio::task::spawn(async move {
                // new connection
                let c = Connection::new(stream);
                // parse header
                let incoming_packet = match c.read_mqtt_frames() {
                    Ok(incoming) => incoming,
                    Err(err) => {
                        return anyhow::anyhow!("Connection Protocol Error {}", err);
                    }
                };
                match incoming_packet.mqttpacket.control_packet {
                    mqtt::ControlPacket::CONNECT(connect) => {
                        match connection_handler.call(incoming_packet).await {
                            // command(serviceによるreturn)を処理する
                            Ok(response) => self.execute_operation(c, response).await?,
                            Err(error) => {} //handle_error(error, connection),
                        }
                    }
                    _ => {}
                }
            })
        }
    }
    // Responseの内容に応じて実行する
    async fn execute_operation(self, c: Connection, response: Response) {
        match response.command {
            Operation::ReturnPacket => {}
            Operation::NOP => {}
        }
    }
}
