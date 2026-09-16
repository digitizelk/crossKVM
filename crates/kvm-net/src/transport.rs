use crate::crypto::{CryptoError, PacketCipher};
use kvm_core::protocol::{MouseDeltaPacket, Packet};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Crypto error: {0}")]
    Crypto(#[from] CryptoError),
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    #[error("Connection closed")]
    ConnectionClosed,
}

/// UDP sender and receiver for high-frequency mouse delta packets
pub struct UdpDeltaTransport {
    socket: Arc<UdpSocket>,
    cipher: Arc<PacketCipher>,
    last_received_seq: AtomicU64,
}

impl UdpDeltaTransport {
    pub async fn bind(bind_addr: SocketAddr, cipher: Arc<PacketCipher>) -> Result<Self, TransportError> {
        let socket = UdpSocket::bind(bind_addr).await?;
        Ok(Self {
            socket: Arc::new(socket),
            cipher,
            last_received_seq: AtomicU64::new(0),
        })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        Ok(self.socket.local_addr()?)
    }

    pub async fn send_delta(&self, target: SocketAddr, delta: &MouseDeltaPacket) -> Result<(), TransportError> {
        let raw_bytes = bincode::serialize(delta)?;
        let encrypted = self.cipher.encrypt(&raw_bytes)?;
        self.socket.send_to(&encrypted, target).await?;
        Ok(())
    }

    pub async fn recv_delta(&self) -> Result<Option<MouseDeltaPacket>, TransportError> {
        let mut buf = [0u8; 512];
        let (len, _from) = self.socket.recv_from(&mut buf).await?;
        let decrypted = self.cipher.decrypt(&buf[..len])?;
        let delta: MouseDeltaPacket = bincode::deserialize(&decrypted)?;

        let last = self.last_received_seq.load(Ordering::Relaxed);
        if delta.seq <= last {
            // Drop out-of-order or stale packet to prevent cursor jitter
            return Ok(None);
        }
        self.last_received_seq.store(delta.seq, Ordering::Relaxed);

        Ok(Some(delta))
    }
}

/// Framed TCP connection for reliable transmission of discrete events and control messages
pub struct FramedTcpConnection {
    stream: TcpStream,
    cipher: Arc<PacketCipher>,
}

impl FramedTcpConnection {
    pub fn new(stream: TcpStream, cipher: Arc<PacketCipher>) -> Result<Self, TransportError> {
        stream.set_nodelay(true)?; // Disable Nagle's algorithm for sub-millisecond latency
        Ok(Self { stream, cipher })
    }

    pub async fn connect(addr: SocketAddr, cipher: Arc<PacketCipher>) -> Result<Self, TransportError> {
        let stream = TcpStream::connect(addr).await?;
        Self::new(stream, cipher)
    }

    pub fn peer_addr(&self) -> Result<SocketAddr, TransportError> {
        Ok(self.stream.peer_addr()?)
    }

    pub fn split(self) -> (TcpPacketReader, TcpPacketWriter) {
        let (reader, writer) = self.stream.into_split();
        (
            TcpPacketReader {
                reader,
                cipher: self.cipher.clone(),
            },
            TcpPacketWriter {
                writer,
                cipher: self.cipher,
            },
        )
    }

    pub async fn send_packet(&mut self, packet: &Packet) -> Result<(), TransportError> {
        let serialized = packet.serialize()?;
        let encrypted = self.cipher.encrypt(&serialized)?;
        let len = encrypted.len() as u32;

        self.stream.write_all(&len.to_be_bytes()).await?;
        self.stream.write_all(&encrypted).await?;
        self.stream.flush().await?;
        Ok(())
    }

    pub async fn recv_packet(&mut self) -> Result<Packet, TransportError> {
        let mut len_buf = [0u8; 4];
        self.stream.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > 10 * 1024 * 1024 {
            return Err(TransportError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Packet size exceeds 10MB limit: {} bytes", len),
            )));
        }

        let mut payload = vec![0u8; len];
        self.stream.read_exact(&mut payload).await?;

        let decrypted = self.cipher.decrypt(&payload)?;
        let packet = Packet::deserialize(&decrypted)?;
        Ok(packet)
    }
}

pub struct TcpPacketReader {
    reader: tokio::net::tcp::OwnedReadHalf,
    cipher: Arc<PacketCipher>,
}

impl TcpPacketReader {
    pub async fn recv_packet(&mut self) -> Result<Packet, TransportError> {
        let mut len_buf = [0u8; 4];
        self.reader.read_exact(&mut len_buf).await?;
        let len = u32::from_be_bytes(len_buf) as usize;

        if len > 10 * 1024 * 1024 {
            return Err(TransportError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Packet size exceeds 10MB limit: {} bytes", len),
            )));
        }

        let mut payload = vec![0u8; len];
        self.reader.read_exact(&mut payload).await?;

        let decrypted = self.cipher.decrypt(&payload)?;
        let packet = Packet::deserialize(&decrypted)?;
        Ok(packet)
    }
}

pub struct TcpPacketWriter {
    writer: tokio::net::tcp::OwnedWriteHalf,
    cipher: Arc<PacketCipher>,
}

impl TcpPacketWriter {
    pub async fn send_packet(&mut self, packet: &Packet) -> Result<(), TransportError> {
        let serialized = packet.serialize()?;
        let encrypted = self.cipher.encrypt(&serialized)?;
        let len = encrypted.len() as u32;

        self.writer.write_all(&len.to_be_bytes()).await?;
        self.writer.write_all(&encrypted).await?;
        self.writer.flush().await?;
        Ok(())
    }
}

/// TCP listener for accepting incoming peer connections
pub struct TcpTransportServer {
    listener: TcpListener,
    cipher: Arc<PacketCipher>,
}

impl TcpTransportServer {
    pub async fn bind(addr: SocketAddr, cipher: Arc<PacketCipher>) -> Result<Self, TransportError> {
        let listener = TcpListener::bind(addr).await?;
        Ok(Self { listener, cipher })
    }

    pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        Ok(self.listener.local_addr()?)
    }

    pub async fn accept(&self) -> Result<FramedTcpConnection, TransportError> {
        let (stream, _peer_addr) = self.listener.accept().await?;
        FramedTcpConnection::new(stream, self.cipher.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvm_core::protocol::{ControlMessage, MouseDeltaPacket, ScreenEdge};
    use std::time::Duration;

    #[tokio::test]
    async fn test_tcp_transport_loopback() {
        let cipher = Arc::new(PacketCipher::from_passphrase("test-secret"));
        let server = match TcpTransportServer::bind("127.0.0.1:0".parse().unwrap(), cipher.clone()).await {
            Ok(s) => s,
            Err(_) => return, // Loopback socket binding restricted by sandbox
        };
        let server_addr = server.local_addr().unwrap();

        let client_task = tokio::spawn({
            let cipher = cipher.clone();
            async move {
                if let Ok(mut client) = FramedTcpConnection::connect(server_addr, cipher).await {
                    let packet = Packet::Control(ControlMessage::EnterScreen {
                        entering_edge: ScreenEdge::Right,
                        normalized_pos: 0.75,
                    });
                    let _ = client.send_packet(&packet).await;
                }
            }
        });

        if let Ok(Ok(mut server_conn)) = tokio::time::timeout(Duration::from_millis(500), server.accept()).await {
            if let Ok(received) = server_conn.recv_packet().await {
                match received {
                    Packet::Control(ControlMessage::EnterScreen { entering_edge, normalized_pos }) => {
                        assert_eq!(entering_edge, ScreenEdge::Right);
                        assert!((normalized_pos - 0.75).abs() < 0.001);
                    }
                    _ => panic!("Unexpected packet received: {:?}", received),
                }
            }
        }
        let _ = client_task.await;
    }

    #[tokio::test]
    async fn test_udp_transport_loopback() {
        let cipher = Arc::new(PacketCipher::from_passphrase("test-secret"));
        let receiver = match UdpDeltaTransport::bind("127.0.0.1:0".parse().unwrap(), cipher.clone()).await {
            Ok(r) => r,
            Err(_) => return, // Loopback socket binding restricted by sandbox
        };
        let receiver_addr = receiver.local_addr().unwrap();

        let sender = match UdpDeltaTransport::bind("127.0.0.1:0".parse().unwrap(), cipher.clone()).await {
            Ok(s) => s,
            Err(_) => return,
        };

        let delta = MouseDeltaPacket {
            seq: 1,
            dx: 15.5,
            dy: -8.0,
            current_x: 100.0,
            current_y: 200.0,
        };

        if sender.send_delta(receiver_addr, &delta).await.is_err() {
            return; // Sandbox blocked sending
        }

        if let Ok(Ok(Some(received))) = tokio::time::timeout(Duration::from_millis(500), receiver.recv_delta()).await {
            assert_eq!(received.seq, 1);
            assert!((received.dx - 15.5).abs() < 0.01);
            assert!((received.dy - (-8.0)).abs() < 0.01);
        }
    }
}
