#!/bin/bash
# Splitux Installer - Installs Splitux to your system
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PREFIX="${1:-$HOME/.local}"

# Colors
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

info() { echo -e "${GREEN}[splitux]${NC} $1"; }
step() { echo -e "${CYAN}[splitux]${NC} $1"; }

# Check dependencies first
if ! "$SCRIPT_DIR/launch.sh" --check-deps-only 2>/dev/null; then
    "$SCRIPT_DIR/launch.sh" || true
    echo ""
    read -p "Continue installation anyway? [y/N] " -n 1 -r
    echo ""
    [[ ! $REPLY =~ ^[Yy]$ ]] && exit 1
fi

step "Installing Splitux to $PREFIX..."

# Find the binary (release: ./splitux, dev: ./build/splitux)
if [[ -f "$SCRIPT_DIR/splitux" ]]; then
    BINARY="$SCRIPT_DIR/splitux"
    BIN_DIR="$SCRIPT_DIR/bin"
    RES_DIR="$SCRIPT_DIR/assets"
elif [[ -f "$SCRIPT_DIR/build/splitux" ]]; then
    BINARY="$SCRIPT_DIR/build/splitux"
    BIN_DIR="$SCRIPT_DIR/build/bin"
    RES_DIR="$SCRIPT_DIR/assets"
else
    echo "Error: splitux binary not found. Run ./splitux.sh build first."
    exit 1
fi

# Create directories
mkdir -p "$PREFIX/bin"
mkdir -p "$PREFIX/share/splitux/bin"
mkdir -p "$PREFIX/share/splitux/assets"
mkdir -p "$PREFIX/share/applications"
mkdir -p "$PREFIX/share/icons/hicolor/128x128/apps"

# Copy binary
cp "$BINARY" "$PREFIX/bin/"
chmod +x "$PREFIX/bin/splitux"

# Copy gamescope-splitux
if [[ -d "$BIN_DIR" ]]; then
    cp -r "$BIN_DIR/"* "$PREFIX/share/splitux/bin/"
    chmod +x "$PREFIX/share/splitux/bin/"*
fi

# Copy resources
if [[ -d "$RES_DIR" ]]; then
    cp -r "$RES_DIR/"* "$PREFIX/share/splitux/assets/"
fi

# Install icon
if [[ -f "$RES_DIR/icon.png" ]]; then
    cp "$RES_DIR/icon.png" "$PREFIX/share/icons/hicolor/128x128/apps/splitux.png"
fi

# Install desktop file
cat > "$PREFIX/share/applications/splitux.desktop" <<EOF
[Desktop Entry]
Name=Splitux
Comment=Local co-op split-screen gaming for Linux
Exec=$PREFIX/bin/splitux
Icon=splitux
Terminal=false
Type=Application
Categories=Game;
Keywords=splitscreen;couch;coop;gaming;multiplayer;
EOF

# Update icon cache if possible
gtk-update-icon-cache "$PREFIX/share/icons/hicolor" 2>/dev/null || true

info "Installation complete!"
echo ""
echo "Splitux installed to: $PREFIX/bin/splitux"
echo ""
if [[ "$PREFIX" == "$HOME/.local" ]]; then
    if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
        echo "Note: Add ~/.local/bin to your PATH if not already:"
        echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
        echo ""
    fi
fi
echo "Launch from your app menu or run: splitux"
