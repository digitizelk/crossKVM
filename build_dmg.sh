#!/bin/bash
set -e

DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$DIR"

echo "=========================================================="
echo "           Cross-KVM macOS .DMG Builder"
echo "=========================================================="

# Ensure release binary is compiled
if [ ! -f "target/release/kvm-daemon" ]; then
    echo "Compiling release binary..."
    cargo build --release -p kvm-daemon
fi

# Ensure staging bundle is ready
mkdir -p dist/Cross-KVM.app/Contents/MacOS
mkdir -p dist/Cross-KVM.app/Contents/Resources

cp target/release/kvm-daemon dist/Cross-KVM.app/Contents/MacOS/kvm-daemon
if [ -f "AppIcon.icns" ]; then
    cp AppIcon.icns dist/Cross-KVM.app/Contents/Resources/AppIcon.icns
fi

# Create Applications symlink for drag-and-drop installer
if [ ! -L dist/Applications ]; then
    rm -rf dist/Applications
    ln -s /Applications dist/Applications
fi

rm -f Cross-KVM.dmg Cross-KVM.dmg.tmp

echo "Creating compressed Apple Disk Image (Cross-KVM.dmg)..."
hdiutil create -volname "Cross-KVM" -srcfolder dist -ov -format UDZO Cross-KVM.dmg

echo "=========================================================="
echo "SUCCESS: Created macOS installer:"
echo "  $(pwd)/Cross-KVM.dmg"
echo "=========================================================="
