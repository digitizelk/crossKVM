use kvm_core::protocol::{ClipboardFormat, ControlMessage};
use kvm_platform::ClipboardHandler;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::{mpsc, Mutex};
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum ClipboardError {
    #[error("Platform clipboard error: {0}")]
    Platform(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Checksum mismatch on file transfer")]
    ChecksumMismatch,
}

pub struct ClipboardService {
    handler: Box<dyn ClipboardHandler>,
    last_change_count: u64,
    cached_payloads: Arc<Mutex<HashMap<[u8; 32], (ClipboardFormat, Vec<u8>)>>>,
}

impl ClipboardService {
    pub fn new(handler: Box<dyn ClipboardHandler>) -> Self {
        let initial_count = handler.get_change_count();
        Self {
            handler,
            last_change_count: initial_count,
            cached_payloads: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Check if local clipboard has changed; returns notification message if changed
    pub async fn poll_changes(&mut self) -> Option<ControlMessage> {
        let current_count = self.handler.get_change_count();
        if current_count == self.last_change_count {
            return None;
        }

        self.last_change_count = current_count;

        // Try reading text first
        if let Ok(Some(text)) = self.handler.read_text() {
            let bytes = text.into_bytes();
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            let sha256: [u8; 32] = hasher.finalize().into();

            let format = ClipboardFormat::PlainText;
            let size = bytes.len();

            let mut cache = self.cached_payloads.lock().await;
            cache.insert(sha256, (format.clone(), bytes));

            return Some(ControlMessage::ClipboardNotify {
                format,
                data_size: size,
                sha256,
            });
        }

        // Try reading image
        if let Ok(Some(png_bytes)) = self.handler.read_image() {
            let mut hasher = Sha256::new();
            hasher.update(&png_bytes);
            let sha256: [u8; 32] = hasher.finalize().into();

            let format = ClipboardFormat::PngImage;
            let size = png_bytes.len();

            let mut cache = self.cached_payloads.lock().await;
            cache.insert(sha256, (format.clone(), png_bytes));

            return Some(ControlMessage::ClipboardNotify {
                format,
                data_size: size,
                sha256,
            });
        }

        None
    }

    /// Handle request for payload by hash
    pub async fn get_payload(&self, sha256: &[u8; 32]) -> Option<ControlMessage> {
        let cache = self.cached_payloads.lock().await;
        if let Some((format, data)) = cache.get(sha256) {
            Some(ControlMessage::ClipboardPayload {
                sha256: *sha256,
                format: format.clone(),
                data: data.clone(),
            })
        } else {
            None
        }
    }

    /// Apply remote payload to local clipboard
    pub fn apply_payload(&mut self, format: &ClipboardFormat, data: &[u8]) -> Result<(), ClipboardError> {
        match format {
            ClipboardFormat::PlainText => {
                let text = String::from_utf8_lossy(data);
                self.handler
                    .write_text(&text)
                    .map_err(|e| ClipboardError::Platform(e.to_string()))?;
                self.last_change_count = self.handler.get_change_count();
                info!("Updated local clipboard with remote text ({} bytes)", data.len());
            }
            ClipboardFormat::PngImage => {
                self.handler
                    .write_image(data)
                    .map_err(|e| ClipboardError::Platform(e.to_string()))?;
                self.last_change_count = self.handler.get_change_count();
                info!("Updated local clipboard with remote image ({} bytes)", data.len());
            }
            _ => {
                warn!("Unsupported clipboard format for direct injection");
            }
        }
        Ok(())
    }
}

/// Drag-and-drop file transfer manager
pub struct FileDropTransfer {
    incoming_dir: PathBuf,
}

impl FileDropTransfer {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let incoming_dir = PathBuf::from(home).join("Downloads").join("ShareDrop");
        let _ = std::fs::create_dir_all(&incoming_dir);
        Self { incoming_dir }
    }

    pub fn target_directory(&self) -> &Path {
        &self.incoming_dir
    }

    /// Prepare to send a file in chunks
    pub async fn read_chunks_and_send(
        path: &Path,
        transfer_id: u64,
        sender: mpsc::Sender<ControlMessage>,
    ) -> Result<(), ClipboardError> {
        let mut file = File::open(path).await?;
        let metadata = file.metadata().await?;
        let file_name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024]; // 64KB chunks

        // Calculate hash
        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        let sha256: [u8; 32] = hasher.finalize().into();

        // Rewind file to start
        let mut file = File::open(path).await?;

        // Send init message
        let _ = sender
            .send(ControlMessage::FileTransferInit {
                transfer_id,
                file_name,
                total_size: metadata.len(),
                sha256,
            })
            .await;

        let mut offset = 0u64;
        loop {
            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }
            let chunk_data = buffer[..n].to_vec();
            let _ = sender
                .send(ControlMessage::FileTransferChunk {
                    transfer_id,
                    offset,
                    data: chunk_data,
                })
                .await;
            offset += n as u64;
        }

        let _ = sender
            .send(ControlMessage::FileTransferDone { transfer_id })
            .await;
        Ok(())
    }
}
