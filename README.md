# Cross-KVM (ShareMouse Alternative)

A modern, cross-platform, peer-to-peer software KVM built in Rust for seamless mouse, keyboard, and clipboard sharing across **macOS**, **Windows**, and **Linux** on the same local network.

---

## Key Features

- **Symmetrical Peer-to-Peer Control**: Any mouse or keyboard attached to any machine can control any other machine on your desk. No fixed server or client designation.
- **Microsecond Latency Dual-Channel Transport**:
  - **UDP Stream**: Continuous high-frequency mouse delta tracking with sequence ordering to prevent jitter/rubberbanding.
  - **TCP Channel**: Ordered, reliable event delivery with `TCP_NODELAY` for clicks, wheel scrolling, and keypresses.
- **Military-Grade Security**: All packets are authenticated and encrypted using **AES-256-GCM** derived from your pre-shared passphrase.
- **Boundary & Topology Physics**: Automatic cursor jumping across screen borders with normalized ratio scaling ($y_{ratio} = y / h$), corner dead-zones (protects window close/minimize buttons), and adjustable edge push thresholds.
- **Physical Preemption Takeover**: If you touch the physical mouse or keyboard of a controlled machine, it immediately reclaims local control.
- **Visual Display Arrangement Dashboard**: Built-in interactive drag-and-drop canvas (runs at `http://localhost:24803`) to arrange monitors just like macOS and Windows display settings.
- **Shared Clipboard & File Drag-and-Drop**: Automatic synchronization of plain text, rich text, and chunked file transfers to `~/Downloads/ShareDrop/`.
- **Emergency Breakout Hotkey**: Press `Ctrl + Alt + Shift + Escape` at any time to instantly release and return the cursor to your local machine.

---

## Quickstart

### 1. Build the Binary
```bash
cargo build --release -p kvm-daemon
```
The compiled binary will be located at `target/release/kvm-daemon`.

---

### 2. Running on macOS
On macOS, ensure Accessibility and Input Monitoring permissions are granted in **System Settings > Privacy & Security**:
```bash
./target/release/kvm-daemon --peer-id mac-host --secret "my-secret-key"
```

To configure screen neighbors:
```bash
# Example: Windows PC is to the right of your Mac
./target/release/kvm-daemon --peer-id mac-host --right-peer win-pc --secret "my-secret-key"
```

Open the visual arrangement canvas in your browser:
```
http://localhost:24803
```

---

### 3. Running on Windows
Clone or copy this repository to your Windows machine and run:
```powershell
cargo build --release -p kvm-daemon
.\target\release\kvm-daemon.exe --peer-id win-pc --left-peer mac-host --secret "my-secret-key" --connect-peer "192.168.1.105:24801"
```

---

### 4. Single-Machine Loopback Self-Test
You can test the two-instance communication on a single computer:
```bash
./test_dual_kvm.sh
```
This spawns:
- **Host A (Mac Primary)**: UDP `24800`, TCP `24801`, UI `http://localhost:24803`
- **Host B (Simulated Peer)**: UDP `24810`, TCP `24811`, UI `http://localhost:24813`

---

## CLI Options

| Flag | Default | Description |
| :--- | :--- | :--- |
| `-s, --secret` | `default-sharemouse-secret` | Passphrase used to derive AES-256-GCM encryption key |
| `-u, --udp-port` | `24800` | Port for high-frequency mouse delta streaming |
| `-t, --tcp-port` | `24801` | Port for reliable clicks, keystrokes, and control messages |
| `--discovery-port` | `24802` | UDP broadcast port for automatic LAN discovery |
| `--ui-port` | `24803` | Local HTTP port for the Visual Monitor Arrangement dashboard |
| `--peer-id` | `hostname-pid` | Unique identifier for this machine on the network |
| `--right-peer` | `None` | Peer ID of the neighbor computer to your right |
| `--left-peer` | `None` | Peer ID of the neighbor computer to your left |
| `--connect-peer` | `None` | Directly connect to a remote peer address (`IP:PORT`) |
| `--check-permissions` | `false` | Verifies OS Accessibility permissions and exits |

---

## Hotkeys & Emergency Controls

| Action | Shortcut | Description |
| :--- | :--- | :--- |
| **Exit / Kill Daemon** | <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>Q</kbd> | Cleanly stops all input capture, restores cursor, and terminates the daemon. |
| **Exit / Kill Daemon** | <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>C</kbd> | Terminal-style kill hotkey (works even while controlling a remote machine). |
| **Emergency Force Kill** | Double <kbd>Ctrl</kbd> + <kbd>C</kbd> | Pressing `Ctrl + C` twice within 600ms immediately kills the daemon. |
| **Emergency Breakout** | Triple-press <kbd>Escape</kbd> | Tap <kbd>Esc</kbd> 3 times rapidly to snap cursor back to the primary screen center. |
| **Emergency Breakout** | <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>Escape</kbd> | Traditional 4-key breakout: releases remote control and re-centers local cursor. |
