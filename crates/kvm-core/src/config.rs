use serde::{Deserialize, Serialize};
use crate::topology::ScreenTopology;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub peer_id: String,
    pub device_name: String,
    pub udp_port: u16,
    pub tcp_port: u16,
    pub discovery_port: u16,
    pub shared_secret: String,
    pub auto_discover: bool,
    pub topology: ScreenTopology,
}

impl Default for AppConfig {
    fn default() -> Self {
        let hostname = std::env::var("HOSTNAME")
            .or_else(|_| std::env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "kvm-device".to_string());

        let peer_id = format!("{}-{}", hostname, &format!("{:08x}", rand_u32())[0..4]);

        Self {
            peer_id: peer_id.clone(),
            device_name: hostname,
            udp_port: 24800,
            tcp_port: 24801,
            discovery_port: 24802,
            shared_secret: "default-sharemouse-secret".to_string(),
            auto_discover: true,
            topology: ScreenTopology::new(peer_id),
        }
    }
}

fn rand_u32() -> u32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345);
    nanos ^ 0x5bd1e995
}
