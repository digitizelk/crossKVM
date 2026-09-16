use kvm_core::protocol::DisplayInfo;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{error, info};

pub const EMBEDDED_UI_HTML: &str = include_str!("ui.html");

pub struct EmbeddedWebServer {
    port: u16,
    displays: Vec<DisplayInfo>,
}

impl EmbeddedWebServer {
    pub fn new(port: u16, displays: Vec<DisplayInfo>) -> Self {
        Self { port, displays }
    }

    pub async fn start(self) -> Result<(), std::io::Error> {
        let addr: SocketAddr = format!("127.0.0.1:{}", self.port).parse().unwrap();
        let listener = TcpListener::bind(addr).await?;
        info!("Monitor Arrangement Web UI active at: http://localhost:{}", self.port);

        let displays_json = serde_json::to_string(&self.displays).unwrap_or_else(|_| "[]".into());
        let displays_arc = Arc::new(displays_json);

        loop {
            match listener.accept().await {
                Ok((mut stream, _client_addr)) => {
                    let displays_data = displays_arc.clone();
                    tokio::spawn(async move {
                        let mut buf = [0u8; 2048];
                        if let Ok(n) = stream.read(&mut buf).await {
                            if n == 0 {
                                return;
                            }
                            let req = String::from_utf8_lossy(&buf[..n]);
                            let first_line = req.lines().next().unwrap_or("");

                            if first_line.starts_with("GET /api/displays") {
                                let body = displays_data.as_bytes();
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    body.len()
                                );
                                let _ = stream.write_all(resp.as_bytes()).await;
                                let _ = stream.write_all(body).await;
                            } else if first_line.starts_with("GET /api/status") {
                                let body = br#"{"status":"running","peers":["win-desktop"],"encrypted":true}"#;
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    body.len()
                                );
                                let _ = stream.write_all(resp.as_bytes()).await;
                                let _ = stream.write_all(body).await;
                            } else {
                                // Serve the arrangement UI HTML
                                let body = EMBEDDED_UI_HTML.as_bytes();
                                let resp = format!(
                                    "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                    body.len()
                                );
                                let _ = stream.write_all(resp.as_bytes()).await;
                                let _ = stream.write_all(body).await;
                            }
                            let _ = stream.flush().await;
                        }
                    });
                }
                Err(e) => {
                    error!("Embedded UI web server accept error: {}", e);
                    break;
                }
            }
        }
        Ok(())
    }
}
