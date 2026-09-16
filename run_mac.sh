#!/bin/bash
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BIN="$DIR/target/debug/kvm-daemon"

if [ ! -f "$BIN" ]; then
    echo "Building kvm-daemon..."
    cargo build -p kvm-daemon
fi

LOCAL_IP="$(ipconfig getifaddr en0 2>/dev/null || ipconfig getifaddr en1 2>/dev/null || echo "127.0.0.1")"

echo "=========================================================="
echo "        Cross-KVM Mac Host Runner"
echo "=========================================================="
echo "Mac Local IP Address: $LOCAL_IP"
echo "Web UI Dashboard:     http://localhost:24803"
echo "Hotkeys:"
echo "  • Exit / Kill:       Ctrl + Alt + Q  or  Ctrl + Alt + C"
echo "  • Emergency Breakout: Triple-press Escape"
echo "=========================================================="

read -p "Is your Windows PC to the [R]ight or [L]eft of your Mac? [R/L, default: R]: " POS
POS="${POS:-R}"

if [[ "$POS" =~ ^[Rr] ]]; then
    echo "Starting Mac Host (Windows is to the RIGHT)..."
    RUST_LOG=info "$BIN" --peer-id "mac-host" --right-peer "win-pc" --secret "my-secret-key"
else
    echo "Starting Mac Host (Windows is to the LEFT)..."
    RUST_LOG=info "$BIN" --peer-id "mac-host" --left-peer "win-pc" --secret "my-secret-key"
fi
