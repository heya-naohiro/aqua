use crate::mqtt::{self, MqttError};
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

pub struct Response<T>
where
    T: mqtt::AsyncWriter,
{
    command: Operation,
    packet: T,
}

pub struct MqttConnection {
    id: String,
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
    pub async fn read_bytes_until_satisfied(&mut self, num_bytes: usize) -> Result<()> {
        while self.buffer.len() < num_bytes {
            let n = self.stream.read_buf(&mut self.buffer).await?;
            if n == 0 {
                return Err(anyhow!("Unexpected EOF"));
            }
        }
        Ok(())
    }

    /*
    | MQTT Control Packet type, | Flags specific to each MQTT Control Packet type |
    | Remaining Length(1-3)                                                       |
     */
    pub async fn read_mqtt_frames(&mut self) -> Result<MqttIncoming> {
        self.read_bytes_until_satisfied(2).await?;
        let mut com = self.read_fixed_header().await?;
        // ramain lengthが存在するか確認する
        // Publishでデータのサイズを考慮する（大きいデータは今バッファーになくてもよいことにする）

        // move（com.mqttpacket.control_packet）ではなく
        // com.mqttpacket.control_packetを借用する
        // なぜならば
        match &mut com.mqttpacket.control_packet {
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
                    return Err(anyhow!("remain length is not consistent remaining length is {}, but read length is {}", proceed_varheader + proceed_payload, com.mqttpacket.remaining_length));
                }
                return Ok(com);
            }
            mqtt::ControlPacket::DISCONNECT(disconnect) => Err(anyhow!("Not implemented")),

            _ => Err(anyhow!("not expected control packet")),
        }
    }

    pub async fn read_fixed_header(&mut self) -> Result<MqttIncoming> {
        let minimum_fixed_header = 2;
        for n in 1..=3 {
            match mqtt::mqtt::parse_fixed_header(&mut self.buffer) {
                Ok((packet, proceed)) => {
                    self.buffer.advance(proceed);
                    return Ok(MqttIncoming {
                        mqttpacket: packet,
                        state: FrameState::ReadFixedHeader,
                    });
                }
                Err(e) => {
                    if let Some(mqtt_error) = e.downcast_ref::<MqttError>() {
                        match mqtt_error {
                            mqtt::MqttError::InsufficientBytes => {
                                self.read_bytes_until_satisfied(minimum_fixed_header + n)
                                    .await?
                            }
                            _ => {
                                return Err(anyhow::anyhow!("Fixed Header Parse Mqtt Error {}", e));
                            }
                        }
                    } else {
                        return Err(anyhow::anyhow!("Fixed Header Parse Error {}", e));
                    }
                }
            };
        }
        return Err(anyhow::anyhow!("Fixed Header Invalid"));
    }
    /*
    pub async fn write_packet(&mut self, packet: &impl mqtt::WriteBytes) {
        match self.stream.write_buf(packet.to_byte()).await {
            Ok()
        }
    }
    */
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
    async fn run<T, U>(&self, mut connection_handler: T) -> Result<()>
    where
        T: tower::Service<MqttIncoming, Response = Response<U>, Error = Infallible>
            + Send
            + 'static,
        U: mqtt::AsyncWriter + Send + 'static,
    {
        let listener = TcpListener::bind(&self.addr).await?;
        loop {
            let (mut stream, remote_addr) = match listener.accept().await {
                Ok(conn) => conn,
                Err(err) => {
                    eprintln!("Failed to accept connection: {}", err);
                    continue; // 次のループに進む
                }
            };
            tokio::task::spawn(async move {
                // new connection
                let c = Connection::new(stream);
                // parse header
                let incoming_packet = match c.read_mqtt_frames().await {
                    Ok(incoming) => incoming,
                    Err(err) => {
                        eprintln!("Connection Protocol Error: {}", err);
                        return;
                    }
                };
                match incoming_packet.mqttpacket.control_packet {
                    mqtt::ControlPacket::CONNECT(connect) => {
                        match connection_handler.call(incoming_packet).await {
                            // command(serviceによるreturn)を処理する
                            Ok(response) => {
                                self.execute_operation(c, response).await;
                            }
                            Err(error) => {
                                eprintln!("Connect handle error: {:?}", error)
                            } //handle_error(error, connection),
                        }
                    }
                    _ => {}
                }
            });
        }
    }
    // Responseの内容に応じて実行する
    async fn execute_operation<T>(&self, mut c: Connection, mut response: Response<T>) -> Result<()>
    where
        T: mqtt::AsyncWriter,
    {
        match response.command {
            Operation::ReturnPacket => {
                response.packet.write(&mut c.stream).await?;
            }
            Operation::NOP => {}
        }
        Ok(())
    }
}
