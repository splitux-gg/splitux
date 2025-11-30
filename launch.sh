#!/bin/bash
# Splitux Launcher - Run this to start Splitux
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

info() { echo -e "${GREEN}[splitux]${NC} $1"; }
warn() { echo -e "${YELLOW}[splitux]${NC} $1"; }
error() { echo -e "${RED}[splitux]${NC} $1"; exit 1; }

# Detect distro and package manager
detect_distro() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        case "$ID" in
            arch|cachyos|manjaro|endeavouros|garuda) PKG_MGR="pacman" ;;
            fedora|nobara|bazzite|ultramarine) PKG_MGR="dnf" ;;
            ubuntu|pop|linuxmint|elementary|zorin|debian|kali) PKG_MGR="apt" ;;
            opensuse*|suse) PKG_MGR="zypper" ;;
            *) PKG_MGR="unknown" ;;
        esac
    else
        PKG_MGR="unknown"
    fi
}

# Check runtime dependencies
check_deps() {
    local missing=()

    command -v bwrap >/dev/null 2>&1 || missing+=("bubblewrap")
    command -v fuse-overlayfs >/dev/null 2>&1 || missing+=("fuse-overlayfs")
    command -v slirp4netns >/dev/null 2>&1 || missing+=("slirp4netns")

    if [[ ${#missing[@]} -eq 0 ]]; then
        return 0
    fi

    echo ""
    warn "Missing dependencies: ${missing[*]}"
    echo ""
    echo "Install them with:"
    echo ""

    case "$PKG_MGR" in
        pacman)
            echo -e "  ${GREEN}sudo pacman -S ${missing[*]}${NC}"
            ;;
        dnf)
            echo -e "  ${GREEN}sudo dnf install ${missing[*]}${NC}"
            ;;
        apt)
            echo -e "  ${GREEN}sudo apt install ${missing[*]}${NC}"
            ;;
        zypper)
            echo -e "  ${GREEN}sudo zypper install ${missing[*]}${NC}"
            ;;
        *)
            echo -e "  Install: ${missing[*]}"
            echo -e "  (Use your distribution's package manager)"
            ;;
    esac
    echo ""
    return 1
}

# Main
detect_distro

if ! check_deps; then
    exit 1
fi

# Support --check-deps-only for install script
if [[ "${1:-}" == "--check-deps-only" ]]; then
    exit 0
fi

# Find and run the binary
if [[ -f "$SCRIPT_DIR/splitux" ]]; then
    exec "$SCRIPT_DIR/splitux" "$@"
elif [[ -f "$SCRIPT_DIR/build/splitux" ]]; then
    exec "$SCRIPT_DIR/build/splitux" "$@"
else
    error "splitux binary not found. If building from source, run: ./splitux.sh build"
fi
