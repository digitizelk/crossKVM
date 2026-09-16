#!/bin/bash
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$DIR/target/debug/kvm-daemon"

if [ ! -f "$BIN" ]; then
    echo "Binary not found. Building kvm-daemon..."
    cargo build -p kvm-daemon
fi

echo "=========================================================="
echo "      Cross-KVM Dual-Instance Loopback Test Runner        "
echo "=========================================================="
echo "Host A (Mac Primary):"
echo "  - UDP Port: 24800, TCP Port: 24801, UI: http://localhost:24803"
echo "  - Neighbor: [Right Edge] ➔ win-sim"
echo ""
echo "Host B (Simulated Peer):"
echo "  - UDP Port: 24810, TCP Port: 24811, UI: http://localhost:24813"
echo "  - Neighbor: [Left Edge] ➔ mac-host"
echo "=========================================================="
echo "HOTKEYS:"
echo "  • Exit / Kill Daemon:     Ctrl + Alt + Q   or   Ctrl + Alt + C"
echo "                            (or double-press Ctrl + C)"
echo "  • Emergency Breakout:     Triple-press Escape  or  Ctrl + Alt + Shift + Esc"
echo "=========================================================="

cleanup() {
    trap - INT TERM EXIT
    echo ""
    echo "Shutting down Cross-KVM daemon instances..."
    kill -TERM "$PID_A" "$PID_B" 2>/dev/null || true
    sleep 0.4
    kill -KILL "$PID_A" "$PID_B" 2>/dev/null || true
    echo "Cleaned up all processes. Exited."
    exit 0
}

trap cleanup INT TERM EXIT

# Start Host A (Primary Mac Host)
RUST_LOG=info "$BIN" \
    --peer-id "mac-host" \
    --udp-port 24800 \
    --tcp-port 24801 \
    --discovery-port 24802 \
    --ui-port 24803 \
    --right-peer "win-sim" &
PID_A=$!

sleep 1

# Start Host B (Simulated Peer connecting to Host A)
RUST_LOG=info "$BIN" \
    --peer-id "win-sim" \
    --udp-port 24810 \
    --tcp-port 24811 \
    --discovery-port 24812 \
    --ui-port 24813 \
    --left-peer "mac-host" \
    --connect-peer "127.0.0.1:24801" &
PID_B=$!

# Watchdog loop: if either process exits, terminate both and exit back to shell
while kill -0 "$PID_A" 2>/dev/null && kill -0 "$PID_B" 2>/dev/null; do
    sleep 0.5
done

cleanup
