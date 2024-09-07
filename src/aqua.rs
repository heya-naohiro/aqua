use crate::mqtt;
use anyhow::{anyhow, Context, Result};
use bytes::{Buf, BytesMut};
use hyper::body::Incoming;
use std::io::Cursor;
use std::io::{BufRead, BufReader, Write};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufWriter};
use tokio::net::TcpListener;
use tokio::net::TcpStream;

use crate::mqtt::MqttPacket;
pub struct Server {
    addr: str,
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

    pub async fn read_mqtt_frames(&mut self) -> Result<Incoming>{
        let mut buf = Cursor::new(&self.buffer);
        let com = self.read_fixed_header(buf).await?;
        // ramain lengthが存在するか確認する
        // Publishでデータのサイズを考慮する（大きいデータは今バッファーになくてもよいことにする）
        match com.mqttpacket.control_packet {
            mqtt::ControlPacket::CONNECT(connect) => {
                buf.read_exact(buf);
                match connect.parse_variable_header(buf) {
                    Ok(proceed) => {
                        buf.advance(proceed);
                    }
                    Err(_e) => {
                        return 
                    } 
                }
                
            }
            mqtt::ControlPacket::DISCONNECT(disconnect) => {

            }
        }
    }

    // https://zenn.dev/magurotuna/books/tokio-tutorial-ja/viewer/framing
    pub async fn read_fixed_header(&mut self, buf: Cursor<&BytesMut>) -> Result<MqttIncoming> {
        //ここループじゃなくてpollなんちゃらでやる方法を調べる
        loop {
            match mqtt::mqtt::parse_fixed_header(buf) {
                Ok((packet, proceed)) => {
                    buf.advance(proceed);
                    Ok(MqttIncoming {
                        mqttpacket: packet,
                        state: FrameState::ReadFixedHeader,
                    })
                }
                mqtt::MqttError::InsufficientBytes => continue,
                Err(e) => return anyhow::anyhow!("Fixed Header Parse Error {}", e),
            }
        }
    }

    pub async fn read_variable_header(&mut self) -> Result<MqttIncoming> {
        loop {
            match 
        }
    }

}

enum FrameState {
    ReadFixedHeader,
    ReadVariableHeader,
    ReadPayload,
}

pub struct MqttIncoming {
    mqttpacket: mqtt::MqttPacket,
    state: FrameState,
}

impl Server {
    async fn run<T>(self, mut connection_handler: T) -> Result<(), anyhow::Error>
    where
        T: tower::Service<MqttIncoming>,
    {
        let listener = TcpListener::bind(self.addr).await?;
        loop {
            let (mut stream, remote_addr) = listener.accept().await?;
            tokio::task::spawn(async move {
                // new connection
                let c = Connection::new(stream);
                // parse header
                match connection_handler.call(request).await {
                    Ok(response) => execute_operation(connection, response).await?,
                    Err(error) => handle_error(error, connection),
                }
            })
        }
    }

    fn read_mqtt_packet(tcpstream: TcpStream, socket_address: SocketAddr) {}
}
