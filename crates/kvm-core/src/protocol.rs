use serde::{Deserialize, Serialize};

/// Mouse button identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u16),
}

/// Bitflags representing active keyboard modifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct KeyModifiers {
    pub shift: bool,
    pub control: bool,
    pub alt_option: bool,
    pub meta_command: bool,
    pub caps_lock: bool,
}

/// Edges of a monitor where transitions occur
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScreenEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl ScreenEdge {
    pub fn opposite(&self) -> Self {
        match self {
            ScreenEdge::Left => ScreenEdge::Right,
            ScreenEdge::Right => ScreenEdge::Left,
            ScreenEdge::Top => ScreenEdge::Bottom,
            ScreenEdge::Bottom => ScreenEdge::Top,
        }
    }
}

/// Formats supported by the shared clipboard
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClipboardFormat {
    PlainText,
    HtmlText,
    RtfText,
    PngImage,
    FileList(Vec<String>),
}

/// Metadata about a connected display
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub is_primary: bool,
    pub x: i32,
    pub y: i32,
}

/// High-frequency mouse delta packet (sent over UDP with monotonic seq ID)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MouseDeltaPacket {
    pub seq: u64,
    pub dx: f32,
    pub dy: f32,
    pub current_x: f32,
    pub current_y: f32,
}

/// Discrete input events (sent reliably over TCP/QUIC with ordering guaranteed)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum InputEvent {
    /// High-frequency delta move (can also be transported over UDP)
    MouseMoveDelta(MouseDeltaPacket),
    /// Set absolute cursor position on enter
    MouseMoveAbsolute { x: f32, y: f32 },
    /// Mouse button pressed
    MouseDown(MouseButton),
    /// Mouse button released
    MouseUp(MouseButton),
    /// Mouse wheel scroll
    Scroll { delta_x: f32, delta_y: f32 },
    /// Keyboard key pressed
    KeyDown {
        scancode: u32,
        keycode: u32,
        modifiers: KeyModifiers,
    },
    /// Keyboard key released
    KeyUp {
        scancode: u32,
        keycode: u32,
        modifiers: KeyModifiers,
    },
}

/// Control messages for peer coordination and session management
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ControlMessage {
    /// Initial handshake from a connecting peer
    Handshake {
        protocol_version: u32,
        peer_id: String,
        device_name: String,
        os: String,
        udp_port: u16,
        displays: Vec<DisplayInfo>,
    },
    /// Handshake response
    HandshakeAck {
        accepted: bool,
        assigned_peer_id: String,
        udp_port: u16,
        displays: Vec<DisplayInfo>,
        reason: Option<String>,
    },
    /// Heartbeat ping to calculate round-trip latency
    Ping {
        seq: u64,
        timestamp_ms: u64,
    },
    /// Heartbeat response
    Pong {
        seq: u64,
        timestamp_ms: u64,
    },
    /// The local pointer reached a screen edge and is jumping to a remote peer
    EnterScreen {
        entering_edge: ScreenEdge,
        /// Normalized coordinate along that edge from 0.0 to 1.0
        normalized_pos: f32,
    },
    /// The pointer left the remote machine and returned home
    LeaveScreen,
    /// Physical mouse or keyboard moved on a follower machine; follower reclaims local control immediately
    PreemptTakeover {
        peer_id: String,
    },
    /// Clipboard content changed notification (lazy transfer)
    ClipboardNotify {
        format: ClipboardFormat,
        data_size: usize,
        sha256: [u8; 32],
    },
    /// Request the actual clipboard payload
    ClipboardRequest {
        sha256: [u8; 32],
    },
    /// Clipboard data payload transfer
    ClipboardPayload {
        sha256: [u8; 32],
        format: ClipboardFormat,
        data: Vec<u8>,
    },
    /// File drag-and-drop initiated across screen edge
    FileTransferInit {
        transfer_id: u64,
        file_name: String,
        total_size: u64,
        sha256: [u8; 32],
    },
    /// Chunk of file data
    FileTransferChunk {
        transfer_id: u64,
        offset: u64,
        data: Vec<u8>,
    },
    /// File transfer finished
    FileTransferDone {
        transfer_id: u64,
    },
}

/// Top-level network packet wrapper
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Packet {
    Input(InputEvent),
    Control(ControlMessage),
}

impl Packet {
    pub const PROTOCOL_VERSION: u32 = 1;

    /// Serialize packet into binary bytes using bincode
    pub fn serialize(&self) -> Result<Vec<u8>, bincode::Error> {
        bincode::serialize(self)
    }

    /// Deserialize packet from binary bytes
    pub fn deserialize(bytes: &[u8]) -> Result<Self, bincode::Error> {
        bincode::deserialize(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_serialization_roundtrip() {
        let packet = Packet::Input(InputEvent::KeyDown {
            scancode: 56,
            keycode: 12,
            modifiers: KeyModifiers {
                shift: true,
                control: false,
                alt_option: true,
                meta_command: false,
                caps_lock: false,
            },
        });

        let bytes = packet.serialize().expect("serialization should succeed");
        let decoded = Packet::deserialize(&bytes).expect("deserialization should succeed");
        assert_eq!(packet, decoded);
    }

    #[test]
    fn test_control_message_roundtrip() {
        let msg = Packet::Control(ControlMessage::EnterScreen {
            entering_edge: ScreenEdge::Left,
            normalized_pos: 0.42,
        });

        let bytes = msg.serialize().unwrap();
        let decoded = Packet::deserialize(&bytes).unwrap();
        assert_eq!(msg, decoded);
    }
}
