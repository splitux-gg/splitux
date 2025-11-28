#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="$SCRIPT_DIR/build"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

info() { echo -e "${GREEN}[splitux]${NC} $1"; }
warn() { echo -e "${YELLOW}[splitux]${NC} $1"; }
error() { echo -e "${RED}[splitux]${NC} $1"; exit 1; }
step() { echo -e "${CYAN}[splitux]${NC} $1"; }

# Get cargo target directory
get_target_dir() {
    if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
        echo "$CARGO_TARGET_DIR"
    elif [[ -f "$HOME/.cache/cargo/target/release/splitux" ]]; then
        echo "$HOME/.cache/cargo/target"
    else
        echo "$SCRIPT_DIR/target"
    fi
}

check_deps() {
    local missing=()
    local optional=()
    local mode="${1:-runtime}"

    # Required runtime dependencies
    command -v fuse-overlayfs >/dev/null 2>&1 || missing+=("fuse-overlayfs")
    command -v bwrap >/dev/null 2>&1 || missing+=("bubblewrap")

    # Build dependencies
    if [[ "$mode" == "build" ]]; then
        command -v cargo >/dev/null 2>&1 || missing+=("cargo/rust")
    fi

    # Optional but recommended
    command -v gamescope >/dev/null 2>&1 || optional+=("gamescope")
    command -v umu-run >/dev/null 2>&1 || optional+=("umu-launcher (for Windows games)")
    command -v premake5 >/dev/null 2>&1 || optional+=("premake5 (for goldberg/Steam LAN)")

    if [[ ${#missing[@]} -gt 0 ]]; then
        error "Missing required dependencies: ${missing[*]}"
    fi

    if [[ ${#optional[@]} -gt 0 && "$mode" == "build" ]]; then
        warn "Optional dependencies not found: ${optional[*]}"
    fi

    info "Dependencies OK"
}

init_submodules() {
    cd "$SCRIPT_DIR"
    if git submodule status | grep -q '^-'; then
        step "Initializing submodules..."
        git submodule update --init --recursive
    fi
}

build_goldberg() {
    local gbe_src="$SCRIPT_DIR/deps/gbe_fork"
    local gbe_out="$SCRIPT_DIR/res/goldberg"

    if [[ -d "$gbe_out/linux64" ]]; then
        info "goldberg already built"
        return 0
    fi

    # Init submodule if needed
    if [[ ! -f "$gbe_src/premake5.lua" ]]; then
        step "Initializing gbe_fork submodule..."
        cd "$SCRIPT_DIR"
        git submodule update --init deps/gbe_fork
    fi

    # Check for premake5
    if ! command -v premake5 >/dev/null 2>&1; then
        warn "premake5 not installed - goldberg (Steam LAN emulator) will not be built"
        warn "Install premake5 or disable 'Emulate Steam Client' in game settings"
        return 1
    fi

    step "Building goldberg Steam emulator..."
    cd "$gbe_src"
    ./build_linux_premake.sh || { warn "goldberg build failed"; return 1; }

    # Copy built libraries
    mkdir -p "$gbe_out/linux32" "$gbe_out/linux64"
    cp -f build/linux/x32/release/*.so "$gbe_out/linux32/" 2>/dev/null || true
    cp -f build/linux/x64/release/*.so "$gbe_out/linux64/" 2>/dev/null || true

    info "goldberg built"
}

build_gamescope() {
    local gsc_src="$SCRIPT_DIR/deps/gamescope"
    local gsc_build="$gsc_src/build-gcc"

    # Already built?
    if [[ -f "$gsc_build/src/gamescope" ]]; then
        info "gamescope-kbm already built"
        return 0
    fi

    # Try system gamescope first
    for path in /usr/bin/gamescope /usr/local/bin/gamescope /usr/sbin/gamescope; do
        if [[ -x "$path" ]]; then
            info "Using system gamescope: $path"
            mkdir -p "$SCRIPT_DIR/deps/gamescope/build-gcc/src"
            ln -sf "$path" "$SCRIPT_DIR/deps/gamescope/build-gcc/src/gamescope"
            return 0
        fi
    done

    # Build from source as fallback
    if [[ ! -f "$gsc_src/meson.build" ]]; then
        step "Initializing gamescope submodule..."
        cd "$SCRIPT_DIR"
        git submodule update --init deps/gamescope
        cd "$gsc_src"
        git submodule update --init --recursive
    fi

    step "Building gamescope-kbm from source..."
    cd "$gsc_src"
    if meson setup build-gcc --buildtype=release \
        -Dpipewire=disabled -Drt_cap=disabled \
        -Ddrm_backend=disabled -Dsdl2_backend=enabled \
        -Denable_openvr_support=false 2>/dev/null && \
       ninja -C build-gcc -j"$(nproc)" 2>/dev/null; then
        info "gamescope-kbm built from source"
    else
        warn "gamescope-kbm build failed and no system gamescope found"
        return 1
    fi
}

build_splitux() {
    step "Building splitux..."
    cd "$SCRIPT_DIR"
    cargo build --release -j"$(nproc)"

    local target_dir=$(get_target_dir)
    if [[ ! -f "$target_dir/release/splitux" ]]; then
        error "Binary not found at $target_dir/release/splitux"
    fi
    info "splitux built"
}

do_build() {
    check_deps build

    # Build all components in parallel
    step "Building components in parallel..."
    build_gamescope &
    local gsc_pid=$!
    build_goldberg &
    local gbe_pid=$!
    build_splitux &
    local spx_pid=$!

    # Wait for all
    wait $gsc_pid || warn "gamescope build failed"
    wait $gbe_pid || warn "goldberg build failed"
    wait $spx_pid || error "splitux build failed"

    # Setup build directory
    step "Setting up build directory..."
    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR/res" "$BUILD_DIR/bin"

    local target_dir=$(get_target_dir)
    cp "$target_dir/release/splitux" "$BUILD_DIR/"
    cp "$SCRIPT_DIR/LICENSE" "$BUILD_DIR/" 2>/dev/null || true
    cp "$SCRIPT_DIR/COPYING.md" "$BUILD_DIR/thirdparty.txt" 2>/dev/null || true

    # Copy all resources (icons, scripts, templates, etc.)
    cp -r "$SCRIPT_DIR/res/"* "$BUILD_DIR/res/" 2>/dev/null || true

    # Gamescope-kbm
    if [[ -f "$SCRIPT_DIR/deps/gamescope/build-gcc/src/gamescope" ]]; then
        cp "$SCRIPT_DIR/deps/gamescope/build-gcc/src/gamescope" "$BUILD_DIR/bin/gamescope-kbm"
    else
        warn "gamescope-kbm not available - install system-wide or build manually"
    fi

    # umu-run (for Windows/Proton games)
    if [[ -f "$SCRIPT_DIR/deps/umu-launcher/umu/umu-run" ]]; then
        cp "$SCRIPT_DIR/deps/umu-launcher/umu/umu-run" "$BUILD_DIR/bin/"
    elif ! command -v umu-run >/dev/null 2>&1; then
        warn "umu-run not available - Windows games may not work"
    fi

    info "Build complete: $BUILD_DIR/"
}

do_run() {
    [[ ! -f "$BUILD_DIR/splitux" ]] && do_build
    check_deps runtime
    info "Running splitux..."
    exec "$BUILD_DIR/splitux" "$@"
}

do_install() {
    local prefix="${1:-$HOME/.local}"
    [[ ! -f "$BUILD_DIR/splitux" ]] && do_build

    step "Installing to $prefix..."
    mkdir -p "$prefix/bin" "$prefix/share/splitux"

    cp "$BUILD_DIR/splitux" "$prefix/bin/"
    [[ -d "$BUILD_DIR/res" ]] && cp -r "$BUILD_DIR/res"/* "$prefix/share/splitux/"
    [[ -f "$BUILD_DIR/bin/gamescope-kbm" ]] && cp "$BUILD_DIR/bin/gamescope-kbm" "$prefix/bin/"

    info "Installed to $prefix (ensure $prefix/bin is in PATH)"
}

do_update() {
    cd "$SCRIPT_DIR"
    local branch=$(git rev-parse --abbrev-ref HEAD)

    step "Fetching origin/$branch..."
    git fetch origin "$branch" 2>/dev/null || error "Fetch failed"

    local local_head=$(git rev-parse HEAD)
    local remote_head=$(git rev-parse "origin/$branch")

    if [[ "$local_head" == "$remote_head" ]]; then
        info "Up to date ($branch @ ${local_head:0:7})"
    else
        local behind=$(git rev-list --count HEAD..origin/$branch)
        local ahead=$(git rev-list --count origin/$branch..HEAD)
        [[ "$behind" -gt 0 ]] && warn "Behind by $behind commit(s) - run: git pull"
        [[ "$ahead" -gt 0 ]] && info "Ahead by $ahead commit(s)"
    fi

    git submodule status | grep -q '^-' && warn "Submodules not initialized - run: $0 build"
}

do_clean() {
    step "Cleaning..."
    rm -rf "$BUILD_DIR"
    rm -rf "$SCRIPT_DIR/deps/gamescope/build-gcc"
    cargo clean 2>/dev/null || true
    info "Clean complete"
}

usage() {
    cat <<EOF
${GREEN}Splitux${NC} - Build & Run Script

${CYAN}Usage:${NC} $0 <command> [options]

${CYAN}Commands:${NC}
    build       Build splitux and gamescope-kbm (parallel)
    run         Build if needed, then run
    install     Install to ~/.local or specified prefix
    update      Check for updates from remote
    check       Verify dependencies
    clean       Remove all build artifacts

${CYAN}Examples:${NC}
    $0 build                # Build everything
    $0 run                  # Build and run
    $0 install              # Install to ~/.local
    $0 install /usr/local   # System-wide install (needs sudo)
    $0 update               # Check for updates
EOF
}

case "${1:-}" in
    build)   do_build ;;
    run)     shift; do_run "$@" ;;
    install) do_install "${2:-}" ;;
    update)  do_update ;;
    check)   check_deps build ;;
    clean)   do_clean ;;
    *)       usage ;;
esac
