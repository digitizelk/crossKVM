use anyhow::Result;
use clap::Parser;
use kvm_clipboard::ClipboardService;
use kvm_core::protocol::{ControlMessage, InputEvent, Packet, ScreenEdge};
use kvm_core::state::{SessionState, StateAction, StateManager};
use kvm_core::topology::{ScreenBoundary, ScreenTopology};
use kvm_net::crypto::PacketCipher;
use kvm_net::discovery::{DiscoveryBeacon, PeerDiscovery};
use kvm_net::transport::{FramedTcpConnection, TcpTransportServer, UdpDeltaTransport};
use kvm_platform::{create_clipboard_handler, create_input_capturer, create_input_injector, create_screen_manager};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

mod web_ui;
use web_ui::EmbeddedWebServer;

#[derive(Parser, Debug)]
#[command(name = "kvm-daemon", about = "Cross-platform software KVM daemon")]
struct CliArgs {
    #[arg(short, long, default_value = "default-sharemouse-secret")]
    secret: String,

    #[arg(short, long, default_value_t = 24800)]
    udp_port: u16,

    #[arg(short, long, default_value_t = 24801)]
    tcp_port: u16,

    #[arg(long, default_value_t = 24802)]
    discovery_port: u16,

    #[arg(long, default_value_t = 24803)]
    ui_port: u16,

    #[arg(long, default_value_t = true)]
    ui: bool,

    #[arg(long)]
    peer_id: Option<String>,

    #[arg(long)]
    right_peer: Option<String>,

    #[arg(long)]
    left_peer: Option<String>,

    #[arg(long)]
    connect_peer: Option<SocketAddr>,

    #[arg(long)]
    check_permissions: bool,
}

#[derive(Clone)]
struct PeerConnection {
    _peer_id: String,
    tcp_sender: mpsc::Sender<Packet>,
    udp_addr: SocketAddr,
}

#[derive(Default)]
pub struct HotkeyState {
    last_esc_presses: Vec<std::time::Instant>,
    last_ctrl_c_presses: Vec<std::time::Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    None,
    EmergencyBreakout,
    ExitApplication,
}

impl HotkeyState {
    pub fn check(&mut self, event: &InputEvent) -> HotkeyAction {
        if let InputEvent::KeyDown { keycode, modifiers, .. } = event {
            let is_esc = *keycode == 53 || *keycode == 27;
            let is_q = *keycode == 12 || *keycode == 81;
            let is_c = *keycode == 8 || *keycode == 67;
            let is_x = *keycode == 7 || *keycode == 88;

            let now = std::time::Instant::now();

            // 1. Check Exit combinations (immediate shutdown of KVM):
            // Ctrl + Alt + Q (Quit)
            if modifiers.control && modifiers.alt_option && is_q {
                return HotkeyAction::ExitApplication;
            }
            // Ctrl + Alt + C (Terminal kill interrupt)
            if modifiers.control && modifiers.alt_option && is_c {
                return HotkeyAction::ExitApplication;
            }
            // Ctrl + Alt + X (Exit)
            if modifiers.control && modifiers.alt_option && is_x {
                return HotkeyAction::ExitApplication;
            }
            // Ctrl + Alt + Escape (Emergency Stop)
            if modifiers.control && modifiers.alt_option && !modifiers.shift && is_esc {
                return HotkeyAction::ExitApplication;
            }
            // Cmd + Alt + Q (macOS style quit)
            if modifiers.meta_command && modifiers.alt_option && is_q {
                return HotkeyAction::ExitApplication;
            }
            // Double Ctrl + C within 600ms (emergency terminal kill even during remote control)
            if modifiers.control && is_c {
                self.last_ctrl_c_presses.retain(|t| now.duration_since(*t).as_millis() < 600);
                self.last_ctrl_c_presses.push(now);
                if self.last_ctrl_c_presses.len() >= 2 {
                    self.last_ctrl_c_presses.clear();
                    return HotkeyAction::ExitApplication;
                }
            }

            // 2. Check Emergency Breakout combinations (return cursor to host without stopping daemon):
            // Ctrl + Alt + Shift + Escape (original 4-finger breakout)
            if modifiers.control && modifiers.alt_option && modifiers.shift && is_esc {
                return HotkeyAction::EmergencyBreakout;
            }
            // Triple-press Escape within 800ms
            if is_esc {
                self.last_esc_presses.retain(|t| now.duration_since(*t).as_millis() < 800);
                self.last_esc_presses.push(now);
                if self.last_esc_presses.len() >= 3 {
                    self.last_esc_presses.clear();
                    return HotkeyAction::EmergencyBreakout;
                }
            }
        }
        HotkeyAction::None
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    let args = CliArgs::parse();

    info!("Starting Cross-KVM Daemon (ShareMouse alternative)...");

    let screen_mgr = create_screen_manager();
    let displays = screen_mgr.get_displays()?;
    info!("Detected {} display(s):", displays.len());
    for d in &displays {
        info!("  Display #{}: {}x{} (primary={})", d.id, d.width, d.height, d.is_primary);
    }

    if args.ui {
        let ui_server = EmbeddedWebServer::new(args.ui_port, displays.clone());
        tokio::spawn(async move {
            if let Err(e) = ui_server.start().await {
                error!("UI Server error: {}", e);
            }
        });
    }

    let mut capturer = create_input_capturer();
    if !capturer.check_permissions() {
        warn!("OS input capture permissions are missing!");
        capturer.request_permissions();
        if args.check_permissions {
            info!("Permissions check requested. Please approve Accessibility / Input Monitoring in System Settings.");
            return Ok(());
        }
    } else {
        info!("OS input capture permissions: GRANTED");
    }

    let cipher = Arc::new(PacketCipher::from_passphrase(&args.secret));

    let local_peer_id = args.peer_id.unwrap_or_else(|| {
        let hostname = std::env::var("HOSTNAME").unwrap_or_else(|_| "host".into());
        format!("{}-{}", hostname, std::process::id())
    });

    let mut topology = ScreenTopology::new(&local_peer_id);
    topology.local_displays = displays.clone();

    if let Some(ref right) = args.right_peer {
        topology.add_neighbor(right, ScreenEdge::Right);
        info!("Topology: [Right Edge] connects to neighbor [{}]", right);
    }
    if let Some(ref left) = args.left_peer {
        topology.add_neighbor(left, ScreenEdge::Left);
        info!("Topology: [Left Edge] connects to neighbor [{}]", left);
    }

    let primary = displays
        .iter()
        .find(|d| d.is_primary)
        .cloned()
        .unwrap_or_else(|| displays[0].clone());

    let center_x = primary.x as f32 + (primary.width as f32 / 2.0);
    let center_y = primary.y as f32 + (primary.height as f32 / 2.0);

    let boundary = Arc::new(ScreenBoundary::from_display(&primary, 3.0, 15.0));
    let state_mgr = Arc::new(Mutex::new(StateManager::new()));
    let (event_tx, mut event_rx) = mpsc::channel::<InputEvent>(1000);
    let active_peers: Arc<Mutex<HashMap<String, PeerConnection>>> = Arc::new(Mutex::new(HashMap::new()));

    let injector = Arc::new(Mutex::new(create_input_injector()));

    let (shutdown_tx, mut shutdown_rx) = mpsc::channel::<()>(1);
    let shutdown_tx_hook = shutdown_tx.clone();
    let hotkey_detector = Arc::new(std::sync::Mutex::new(HotkeyState::default()));

    // Setup input hook callback
    let state_mgr_clone = state_mgr.clone();
    let event_tx_clone = event_tx.clone();
    let boundary_clone = boundary.clone();
    let topology_arc = Arc::new(Mutex::new(topology));
    let topology_clone = topology_arc.clone();
    let active_peers_clone = active_peers.clone();
    let injector_clone = injector.clone();

    info!("Registering OS low-level input hooks...");
    let capture_res = capturer.start_capture(Box::new(move |event| {
        // 1. Check Global Hotkeys (Exit or Breakout)
        let action = if let Ok(mut det) = hotkey_detector.lock() {
            det.check(event)
        } else {
            HotkeyAction::None
        };

        if action == HotkeyAction::ExitApplication {
            info!("🛑 EXIT HOTKEY TRIGGERED (Ctrl+Alt+Q / Ctrl+Alt+C / Double Ctrl+C). Shutting down Cross-KVM...");
            if let Ok(mut inj) = injector_clone.try_lock() {
                let _ = inj.warp_cursor(center_x, center_y);
                let _ = inj.show_cursor();
            }
            if let Ok(peers) = active_peers_clone.try_lock() {
                for peer in peers.values() {
                    let _ = peer.tcp_sender.try_send(Packet::Control(ControlMessage::LeaveScreen));
                }
            }
            let _ = shutdown_tx_hook.try_send(());
            return false;
        }

        if action == HotkeyAction::EmergencyBreakout {
            if let Ok(mut mgr) = state_mgr_clone.try_lock() {
                mgr.return_to_local();
                if let Ok(mut inj) = injector_clone.try_lock() {
                    let _ = inj.warp_cursor(center_x, center_y);
                    let _ = inj.show_cursor();
                }
                if let Ok(peers) = active_peers_clone.try_lock() {
                    for peer in peers.values() {
                        let _ = peer.tcp_sender.try_send(Packet::Control(ControlMessage::LeaveScreen));
                    }
                }
                info!("🚨 EMERGENCY BREAKOUT: Returned cursor to primary screen center.");
                return false;
            }
        }

        let mut state = match state_mgr_clone.try_lock() {
            Ok(s) => s,
            Err(_) => return false,
        };

        // 2. If controlling remote peer: stream all inputs to remote machine and swallow locally
        if state.is_controlling_remote() {
            let _ = event_tx_clone.try_send(event.clone());
            return true; // Swallow locally!
        }

        // 3. If remote controlled: detect physical local movement and preempt takeover!
        if state.is_remote_controlled() {
            let action = state.handle_physical_input_detected();
            if let StateAction::NotifyPreempt { peer_id } = action {
                info!("Local physical peripheral movement detected! Preempting takeover from {}.", peer_id);
                if let Ok(mut inj) = injector_clone.try_lock() {
                    let _ = inj.show_cursor();
                }
                if let Ok(peers) = active_peers_clone.try_lock() {
                    if let Some(peer) = peers.get(&peer_id) {
                        let _ = peer.tcp_sender.try_send(Packet::Control(ControlMessage::PreemptTakeover {
                            peer_id: peer_id.clone(),
                        }));
                    }
                }
            }
            return false;
        }

        // 4. If local active: check screen boundary edge hit!
        if state.is_local_active() {
            if let InputEvent::MouseMoveDelta(delta) = event {
                if let Some((edge, ratio)) = boundary_clone.check_edge_hit(delta.current_x, delta.current_y) {
                    if let Ok(topo) = topology_clone.try_lock() {
                        if let Some(neighbor) = topo.find_neighbor_at_edge(edge) {
                            let target_peer = neighbor.peer_id.clone();
                            drop(topo);

                            state.start_controlling_remote(target_peer.clone());
                            if let Ok(mut inj) = injector_clone.try_lock() {
                                let _ = inj.hide_cursor();
                            }
                            info!("BORDER HIT: Screen edge {:?} hit at ratio {:.2}. Jumping to peer [{}]", edge, ratio, target_peer);

                            if let Ok(peers) = active_peers_clone.try_lock() {
                                if let Some(peer) = peers.get(&target_peer) {
                                    let _ = peer.tcp_sender.try_send(Packet::Control(ControlMessage::EnterScreen {
                                        entering_edge: edge.opposite(),
                                        normalized_pos: ratio,
                                    }));
                                }
                            }
                            return true; // Swallow immediately on jump
                        }
                    }
                }
            }
        }

        false
    }));

    if let Err(e) = capture_res {
        warn!("Failed to start hook capture: {}. (Operating in receiver/injection mode).", e);
    } else {
        info!("Input hooks actively monitoring system events.");
    }

    // Start UDP delta socket
    let udp_addr: SocketAddr = format!("0.0.0.0:{}", args.udp_port).parse()?;
    let udp_transport = Arc::new(UdpDeltaTransport::bind(udp_addr, cipher.clone()).await?);
    info!("UDP delta stream listening on {}", udp_transport.local_addr()?);

    // Start TCP reliable server
    let tcp_addr: SocketAddr = format!("0.0.0.0:{}", args.tcp_port).parse()?;
    let tcp_server = Arc::new(TcpTransportServer::bind(tcp_addr, cipher.clone()).await?);
    info!("TCP reliable server listening on {}", tcp_server.local_addr()?);

    // Start Peer Discovery
    let beacon = DiscoveryBeacon {
        peer_id: local_peer_id.clone(),
        device_name: local_peer_id.clone(),
        os: std::env::consts::OS.to_string(),
        tcp_port: args.tcp_port,
        udp_port: args.udp_port,
        displays: displays.clone(),
    };
    let discovery = PeerDiscovery::new(beacon, args.discovery_port);
    let (peer_tx, mut peer_rx) = mpsc::channel(32);
    tokio::spawn(async move {
        if let Err(e) = discovery.start(peer_tx).await {
            error!("Peer discovery failed: {}", e);
        }
    });

    // Handle peer discovery notifications
    tokio::spawn(async move {
        while let Some((peer, addr)) = peer_rx.recv().await {
            info!("LAN Peer Discovered: {} ({}) at {}", peer.peer_id, peer.os, addr);
        }
    });

    // Start Clipboard service
    let mut clipboard_svc = ClipboardService::new(create_clipboard_handler());
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(500));
        loop {
            interval.tick().await;
            if let Some(msg) = clipboard_svc.poll_changes().await {
                info!("Local clipboard changed: {:?}", msg);
            }
        }
    });

    // Receiver task for incoming UDP mouse movements
    let udp_recv_transport = udp_transport.clone();
    let udp_state_mgr = state_mgr.clone();
    let udp_injector = injector.clone();

    tokio::spawn(async move {
        loop {
            match udp_recv_transport.recv_delta().await {
                Ok(Some(delta)) => {
                    let is_controlled = {
                        let state = udp_state_mgr.lock().await;
                        state.is_remote_controlled()
                    };
                    if is_controlled {
                        let mut inj = udp_injector.lock().await;
                        let _ = inj.inject_event(&InputEvent::MouseMoveDelta(delta));
                    }
                }
                Ok(None) => {} // Stale packet dropped
                Err(e) => {
                    error!("UDP recv error: {}", e);
                    break;
                }
            }
        }
    });

    // Receiver task for incoming TCP peer connections (clicks, keys, boundary transitions)
    let tcp_listener_server = tcp_server.clone();
    let tcp_state_mgr = state_mgr.clone();
    let tcp_injector = injector.clone();
    let tcp_boundary = boundary.clone();
    let tcp_active_peers = active_peers.clone();
    let local_peer_id_clone = local_peer_id.clone();
    let local_udp_port = args.udp_port;

    tokio::spawn(async move {
        loop {
            match tcp_listener_server.accept().await {
                Ok(conn) => {
                    let state_mgr = tcp_state_mgr.clone();
                    let injector = tcp_injector.clone();
                    let boundary = tcp_boundary.clone();
                    let peers = tcp_active_peers.clone();
                    let peer_id = local_peer_id_clone.clone();

                    handle_peer_connection(conn, state_mgr, injector, boundary, peers, peer_id, local_udp_port).await;
                }
                Err(e) => {
                    error!("TCP accept error: {}", e);
                    break;
                }
            }
        }
    });

    // Connect outbound peer if specified via CLI
    if let Some(peer_addr) = args.connect_peer {
        info!("Connecting outbound to peer at {}...", peer_addr);
        let cipher_clone = cipher.clone();
        let state_mgr = state_mgr.clone();
        let injector_clone = injector.clone();
        let boundary_clone = boundary.clone();
        let peers_clone = active_peers.clone();
        let peer_id = local_peer_id.clone();

        tokio::spawn(async move {
            match FramedTcpConnection::connect(peer_addr, cipher_clone).await {
                Ok(conn) => {
                    info!("Successfully connected to peer at {}", peer_addr);
                    handle_peer_connection(conn, state_mgr, injector_clone, boundary_clone, peers_clone, peer_id, local_udp_port).await;
                }
                Err(e) => {
                    error!("Failed to connect to peer at {}: {}", peer_addr, e);
                }
            }
        });
    }

    // Sender task for outgoing captured events when controlling remote
    let active_peers_for_sender = active_peers.clone();
    let udp_send_transport = udp_transport.clone();
    let state_mgr_for_sender = state_mgr.clone();

    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            let target_peer_id = {
                let state = state_mgr_for_sender.lock().await;
                if let SessionState::ControllingRemote { ref target_peer_id } = *state.state() {
                    Some(target_peer_id.clone())
                } else {
                    None
                }
            };

            if let Some(target_id) = target_peer_id {
                let peers = active_peers_for_sender.lock().await;
                let target_peer = peers.get(&target_id).or_else(|| peers.values().next());

                if let Some(peer) = target_peer {
                    match event {
                        InputEvent::MouseMoveDelta(delta) => {
                            let _ = udp_send_transport.send_delta(peer.udp_addr, &delta).await;
                        }
                        discrete_ev => {
                            let _ = peer.tcp_sender.send(Packet::Input(discrete_ev)).await;
                        }
                    }
                }
            }
        }
    });

    info!("============================================================");
    info!("Cross-KVM active! Peer-to-peer sharing enabled.");
    info!("  • Exit / Quit Daemon:      Ctrl + Alt + Q  or  Ctrl + Alt + C");
    info!("                             (or double-press Ctrl + C)");
    info!("  • Emergency Breakout:      Triple-press Escape  or  Ctrl + Alt + Shift + Esc");
    info!("============================================================");

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C interrupt signal. Stopping daemon...");
        }
        _ = shutdown_rx.recv() => {
            info!("Received exit hotkey signal. Stopping daemon...");
        }
    }

    let _ = capturer.stop_capture();
    if let Ok(mut inj) = injector.try_lock() {
        let _ = inj.show_cursor();
    }
    info!("Clean shutdown complete. Exited.");
    std::process::exit(0);
}

async fn handle_peer_connection(
    conn: FramedTcpConnection,
    state_mgr: Arc<Mutex<StateManager>>,
    injector: Arc<Mutex<Box<dyn kvm_platform::InputInjector>>>,
    boundary: Arc<ScreenBoundary>,
    active_peers: Arc<Mutex<HashMap<String, PeerConnection>>>,
    local_peer_id: String,
    local_udp_port: u16,
) {
    let peer_ip = conn.peer_addr().map(|a| a.ip()).unwrap_or_else(|_| "127.0.0.1".parse().unwrap());
    let (tx, mut rx) = mpsc::channel::<Packet>(100);
    let (mut reader, mut writer) = conn.split();

    // Send initial handshake
    let handshake_packet = Packet::Control(ControlMessage::Handshake {
        protocol_version: 1,
        peer_id: local_peer_id.clone(),
        device_name: local_peer_id.clone(),
        os: std::env::consts::OS.to_string(),
        udp_port: local_udp_port,
        displays: Vec::new(),
    });
    let _ = tx.send(handshake_packet).await;

    // Independent writer task
    tokio::spawn(async move {
        while let Some(outgoing) = rx.recv().await {
            if let Err(e) = writer.send_packet(&outgoing).await {
                error!("Failed to send outgoing packet to peer: {}", e);
                break;
            }
        }
    });

    // Independent reader task
    let local_peer_id_clone = local_peer_id.clone();
    let tx_for_reply = tx.clone();

    tokio::spawn(async move {
        let mut current_registered_peer: Option<String> = None;

        loop {
            match reader.recv_packet().await {
                Ok(Packet::Input(input_ev)) => {
                    let mut inj = injector.lock().await;
                    let _ = inj.inject_event(&input_ev);
                }
                Ok(Packet::Control(ctrl)) => match ctrl {
                    ControlMessage::Handshake { peer_id, udp_port, .. } => {
                        let udp_target = SocketAddr::new(peer_ip, udp_port);
                        info!("Handshake received from peer [{}] (UDP target: {})", peer_id, udp_target);

                        let mut peers = active_peers.lock().await;
                        peers.insert(
                            peer_id.clone(),
                            PeerConnection {
                                _peer_id: peer_id.clone(),
                                tcp_sender: tx_for_reply.clone(),
                                udp_addr: udp_target,
                            },
                        );
                        current_registered_peer = Some(peer_id.clone());

                        // Send HandshakeAck
                        let _ = tx_for_reply.send(Packet::Control(ControlMessage::HandshakeAck {
                            accepted: true,
                            assigned_peer_id: local_peer_id_clone.clone(),
                            udp_port: local_udp_port,
                            displays: Vec::new(),
                            reason: None,
                        })).await;
                    }
                    ControlMessage::HandshakeAck { assigned_peer_id, udp_port, .. } => {
                        let udp_target = SocketAddr::new(peer_ip, udp_port);
                        info!("Handshake acknowledged by peer [{}] (UDP target: {})", assigned_peer_id, udp_target);

                        let mut peers = active_peers.lock().await;
                        peers.insert(
                            assigned_peer_id.clone(),
                            PeerConnection {
                                _peer_id: assigned_peer_id.clone(),
                                tcp_sender: tx_for_reply.clone(),
                                udp_addr: udp_target,
                            },
                        );
                        current_registered_peer = Some(assigned_peer_id);
                    }
                    ControlMessage::EnterScreen { entering_edge, normalized_pos } => {
                        info!("Peer entered screen from edge {:?} at ratio {:.2}", entering_edge, normalized_pos);
                        let (x, y) = boundary.calculate_entry_point(entering_edge, normalized_pos);
                        let mut inj = injector.lock().await;
                        let _ = inj.warp_cursor(x, y);
                        let _ = inj.show_cursor();

                        let mut state = state_mgr.lock().await;
                        state.enter_remote_controlled("remote-peer".into());
                    }
                    ControlMessage::LeaveScreen => {
                        info!("Peer left screen. Returning to local control.");
                        let mut state = state_mgr.lock().await;
                        state.return_to_local();
                        let mut inj = injector.lock().await;
                        let _ = inj.show_cursor();
                    }
                    ControlMessage::PreemptTakeover { peer_id } => {
                        info!("Peer {} preempted control. Returning to local control.", peer_id);
                        let mut state = state_mgr.lock().await;
                        state.return_to_local();
                        let mut inj = injector.lock().await;
                        let _ = inj.show_cursor();
                    }
                    _ => {}
                },
                Err(e) => {
                    info!("Peer disconnected: {}", e);
                    if let Some(id) = current_registered_peer {
                        let mut peers = active_peers.lock().await;
                        peers.remove(&id);
                    }
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use kvm_core::protocol::KeyModifiers;

    #[test]
    fn test_ctrl_alt_q_exits() {
        let mut hotkeys = HotkeyState::default();
        let ev = InputEvent::KeyDown {
            scancode: 12,
            keycode: 12, // macOS 'Q'
            modifiers: KeyModifiers {
                control: true,
                alt_option: true,
                ..Default::default()
            },
        };
        assert_eq!(hotkeys.check(&ev), HotkeyAction::ExitApplication);
    }

    #[test]
    fn test_ctrl_alt_c_exits() {
        let mut hotkeys = HotkeyState::default();
        let ev = InputEvent::KeyDown {
            scancode: 8,
            keycode: 8, // macOS 'C'
            modifiers: KeyModifiers {
                control: true,
                alt_option: true,
                ..Default::default()
            },
        };
        assert_eq!(hotkeys.check(&ev), HotkeyAction::ExitApplication);
    }

    #[test]
    fn test_double_ctrl_c_exits() {
        let mut hotkeys = HotkeyState::default();
        let ev = InputEvent::KeyDown {
            scancode: 8,
            keycode: 8, // macOS 'C'
            modifiers: KeyModifiers {
                control: true,
                ..Default::default()
            },
        };
        // First Ctrl+C: Normal key
        assert_eq!(hotkeys.check(&ev), HotkeyAction::None);
        // Second Ctrl+C immediately after: Triggers emergency exit!
        assert_eq!(hotkeys.check(&ev), HotkeyAction::ExitApplication);
    }

    #[test]
    fn test_triple_escape_breakout() {
        let mut hotkeys = HotkeyState::default();
        let ev = InputEvent::KeyDown {
            scancode: 53,
            keycode: 53, // macOS Escape
            modifiers: KeyModifiers::default(),
        };
        assert_eq!(hotkeys.check(&ev), HotkeyAction::None);
        assert_eq!(hotkeys.check(&ev), HotkeyAction::None);
        // 3rd rapid escape triggers breakout
        assert_eq!(hotkeys.check(&ev), HotkeyAction::EmergencyBreakout);
    }

    #[test]
    fn test_ctrl_alt_shift_escape_breakout() {
        let mut hotkeys = HotkeyState::default();
        let ev = InputEvent::KeyDown {
            scancode: 53,
            keycode: 53,
            modifiers: KeyModifiers {
                control: true,
                alt_option: true,
                shift: true,
                ..Default::default()
            },
        };
        assert_eq!(hotkeys.check(&ev), HotkeyAction::EmergencyBreakout);
    }
}
