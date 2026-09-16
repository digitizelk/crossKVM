use kvm_core::protocol::DisplayInfo;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::mpsc;
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveryBeacon {
    pub peer_id: String,
    pub device_name: String,
    pub os: String,
    pub tcp_port: u16,
    pub udp_port: u16,
    pub displays: Vec<DisplayInfo>,
}

pub struct PeerDiscovery {
    beacon: DiscoveryBeacon,
    discovery_port: u16,
}

impl PeerDiscovery {
    pub fn new(beacon: DiscoveryBeacon, discovery_port: u16) -> Self {
        Self {
            beacon,
            discovery_port,
        }
    }

    pub async fn start(
        self,
        peer_sender: mpsc::Sender<(DiscoveryBeacon, SocketAddr)>,
    ) -> Result<(), std::io::Error> {
        let listen_addr = format!("0.0.0.0:{}", self.discovery_port);
        let socket = UdpSocket::bind(&listen_addr).await?;
        socket.set_broadcast(true)?;

        let socket = Arc::new(socket);
        let broadcast_socket = socket.clone();
        let beacon_bytes = serde_json::to_vec(&self.beacon).expect("beacon serialization failed");
        let broadcast_target: SocketAddr = format!("255.255.255.255:{}", self.discovery_port)
            .parse()
            .unwrap();

        let local_peer_id = self.beacon.peer_id.clone();

        // Task 1: Periodic broadcast beacon
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                if let Err(e) = broadcast_socket.send_to(&beacon_bytes, broadcast_target).await {
                    error!("Discovery broadcast failed: {}", e);
                }
            }
        });

        // Task 2: Listen for incoming beacons
        let mut buf = vec![0u8; 4096];
        loop {
            match socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    if let Ok(incoming) = serde_json::from_slice::<DiscoveryBeacon>(&buf[..len]) {
                        // Ignore self-announcement
                        if incoming.peer_id != local_peer_id {
                            info!("Discovered peer {} ({}) at {}", incoming.peer_id, incoming.device_name, addr);
                            let _ = peer_sender.send((incoming, addr)).await;
                        }
                    }
                }
                Err(e) => {
                    error!("Discovery socket error: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }
}
