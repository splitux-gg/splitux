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

# =============================================================================
# Distro Detection
# =============================================================================

detect_distro() {
    if [[ -f /etc/os-release ]]; then
        . /etc/os-release
        case "$ID" in
            arch|cachyos|manjaro|endeavouros|garuda)
                DISTRO="arch"
                PKG_MGR="pacman"
                ;;
            fedora|nobara|bazzite|ultramarine)
                DISTRO="fedora"
                PKG_MGR="dnf"
                ;;
            ubuntu|pop|linuxmint|elementary|zorin)
                DISTRO="ubuntu"
                PKG_MGR="apt"
                ;;
            debian|kali|parrot)
                DISTRO="debian"
                PKG_MGR="apt"
                ;;
            opensuse*|suse)
                DISTRO="opensuse"
                PKG_MGR="zypper"
                ;;
            steamos)
                DISTRO="steamos"
                PKG_MGR="flatpak"
                ;;
            *)
                DISTRO="unknown"
                PKG_MGR="unknown"
                ;;
        esac
    else
        DISTRO="unknown"
        PKG_MGR="unknown"
    fi

    # Detect immutable distros
    IMMUTABLE=false
    if [[ -f /run/ostree-booted ]] || [[ "$ID" == "bazzite" ]] || [[ "$ID" == "steamos" ]]; then
        IMMUTABLE=true
    fi
}

# =============================================================================
# Package Maps (distro -> package names)
# =============================================================================

# Maps generic names to distro-specific package names
declare -A PKG_ARCH=(
    [fuse-overlayfs]="fuse-overlayfs"
    [bubblewrap]="bubblewrap"
    [rust]="rust"
    [git]="git"
    [curl]="curl"
    [p7zip]="p7zip"
    [unzip]="unzip"
    [sdl2]="sdl2"
    [libudev]="systemd-libs"
    [meson]="meson"
    [ninja]="ninja"
    # gamescope build deps
    [libxkbcommon]="libxkbcommon"
    [libinput]="libinput"
    [libdrm]="libdrm"
    [vulkan-headers]="vulkan-headers"
    [wayland]="wayland"
    [wayland-protocols]="wayland-protocols"
    [libcap]="libcap"
    [libx11]="libx11"
    [libxres]="libxres"
    [libxcomposite]="libxcomposite"
    [libxtst]="libxtst"
    [libxdamage]="libxdamage"
    [pixman]="pixman"
    [benchmark]="benchmark"
    [glslang]="glslang"
    [stb]="stb"
    [hwdata]="hwdata"
)

declare -A PKG_FEDORA=(
    [gamescope]="gamescope"
    [fuse-overlayfs]="fuse-overlayfs"
    [bubblewrap]="bubblewrap"
    [rust]="rust cargo"
    [git]="git"
    [curl]="curl"
    [p7zip]="p7zip p7zip-plugins"
    [unzip]="unzip"
    [sdl2]="SDL2-devel"
    [libudev]="systemd-devel"
)

declare -A PKG_UBUNTU=(
    [gamescope]="gamescope"
    [fuse-overlayfs]="fuse-overlayfs"
    [bubblewrap]="bubblewrap"
    [rust]="rustc cargo"
    [git]="git"
    [curl]="curl"
    [p7zip]="p7zip-full"
    [unzip]="unzip"
    [sdl2]="libsdl2-dev"
    [libudev]="libudev-dev"
)

declare -A PKG_OPENSUSE=(
    [gamescope]="gamescope"
    [fuse-overlayfs]="fuse-overlayfs"
    [bubblewrap]="bubblewrap"
    [rust]="rust cargo"
    [git]="git"
    [curl]="curl"
    [p7zip]="p7zip"
    [unzip]="unzip"
    [sdl2]="libSDL2-devel"
    [libudev]="libudev-devel"
)

# AUR/COPR/PPA packages (extras not in main repos)
declare -A EXTRA_ARCH=(
    [umu-launcher]="umu-launcher"
)

declare -A EXTRA_FEDORA=(
    [umu-launcher]="umu-launcher"  # Available in COPR or Bazzite repos
)

# =============================================================================
# Package Installation
# =============================================================================

get_pkg_name() {
    local generic="$1"
    case "$DISTRO" in
        arch)    echo "${PKG_ARCH[$generic]:-$generic}" ;;
        fedora)  echo "${PKG_FEDORA[$generic]:-$generic}" ;;
        ubuntu|debian) echo "${PKG_UBUNTU[$generic]:-$generic}" ;;
        opensuse) echo "${PKG_OPENSUSE[$generic]:-$generic}" ;;
        *)       echo "$generic" ;;
    esac
}

install_packages() {
    local packages=("$@")
    [[ ${#packages[@]} -eq 0 ]] && return 0

    case "$PKG_MGR" in
        pacman)
            sudo pacman -S --needed "${packages[@]}"
            ;;
        dnf)
            sudo dnf install -y "${packages[@]}"
            ;;
        apt)
            sudo apt-get update
            sudo apt-get install -y "${packages[@]}"
            ;;
        zypper)
            sudo zypper install -y "${packages[@]}"
            ;;
        flatpak)
            warn "Flatpak-based system - please install packages via your distro's package manager"
            return 1
            ;;
        *)
            error "Unknown package manager: $PKG_MGR"
            ;;
    esac
}

install_aur_package() {
    local pkg="$1"
    if command -v yay >/dev/null 2>&1; then
        yay -S --needed "$pkg"
    elif command -v paru >/dev/null 2>&1; then
        paru -S --needed "$pkg"
    else
        warn "No AUR helper found. Install yay or paru, then run: yay -S $pkg"
        return 1
    fi
}

install_copr_package() {
    local pkg="$1"
    # umu-launcher is in Bazzite/Nobara repos by default
    # For vanilla Fedora, enable COPR
    if ! rpm -q "$pkg" >/dev/null 2>&1; then
        if [[ "$ID" == "fedora" ]]; then
            sudo dnf copr enable -y kylegospo/umu-launcher 2>/dev/null || true
        fi
        sudo dnf install -y "$pkg"
    fi
}

# =============================================================================
# Dependency Checking
# =============================================================================

check_deps() {
    local mode="${1:-runtime}"
    local missing_pkgs=()
    local missing_extra=()

    detect_distro
    step "Detected: $DISTRO ($PKG_MGR)${IMMUTABLE:+ [immutable]}"

    # Runtime dependencies (always needed)
    # Note: gamescope-splitux is built from source, not a package
    local runtime_deps=(fuse-overlayfs bubblewrap)
    for dep in "${runtime_deps[@]}"; do
        local cmd="$dep"
        [[ "$dep" == "bubblewrap" ]] && cmd="bwrap"
        if ! command -v "$cmd" >/dev/null 2>&1; then
            missing_pkgs+=("$(get_pkg_name "$dep")")
        fi
    done

    # Build dependencies
    if [[ "$mode" == "build" ]]; then
        local build_deps=(rust git curl p7zip unzip sdl2 libudev)

        # Check cargo specifically for rust
        if ! command -v cargo >/dev/null 2>&1; then
            missing_pkgs+=("$(get_pkg_name rust)")
        fi

        for dep in git curl unzip; do
            if ! command -v "$dep" >/dev/null 2>&1; then
                missing_pkgs+=("$(get_pkg_name "$dep")")
            fi
        done

        # 7z command
        if ! command -v 7z >/dev/null 2>&1; then
            missing_pkgs+=("$(get_pkg_name p7zip)")
        fi
    fi

    # umu-launcher (for Proton games)
    if ! command -v umu-run >/dev/null 2>&1; then
        missing_extra+=("umu-launcher")
    fi

    # Remove duplicates and empty entries
    local unique_pkgs=($(printf '%s\n' "${missing_pkgs[@]}" | grep -v '^$' | sort -u))
    local unique_extra=($(printf '%s\n' "${missing_extra[@]}" | grep -v '^$' | sort -u))

    if [[ ${#unique_pkgs[@]} -eq 0 ]] && [[ ${#unique_extra[@]} -eq 0 ]]; then
        info "All dependencies OK"
        return 0
    fi

    # Show what's missing
    echo ""
    warn "Missing dependencies:"
    echo ""
    [[ ${#unique_pkgs[@]} -gt 0 ]] && echo -e "  ${CYAN}Packages:${NC} ${unique_pkgs[*]}"
    [[ ${#unique_extra[@]} -gt 0 ]] && echo -e "  ${CYAN}Extra:${NC} ${unique_extra[*]}"
    echo ""

    # Handle immutable distros
    if [[ "$IMMUTABLE" == true ]]; then
        warn "Immutable system detected"
        echo -e "  On Bazzite/SteamOS, most gaming deps are pre-installed."
        echo -e "  If missing, use: ${GREEN}rpm-ostree install <package>${NC}"
        echo ""
        return 1
    fi

    # Prompt to install
    read -p "  Install missing dependencies? [Y/n] " -n 1 -r
    echo ""

    if [[ ! $REPLY =~ ^[Nn]$ ]]; then
        # Install main packages
        if [[ ${#unique_pkgs[@]} -gt 0 ]]; then
            step "Installing packages..."
            install_packages "${unique_pkgs[@]}" || error "Failed to install packages"
        fi

        # Install extra packages (AUR/COPR)
        for pkg in "${unique_extra[@]}"; do
            step "Installing $pkg..."
            case "$DISTRO" in
                arch)
                    install_aur_package "${EXTRA_ARCH[$pkg]:-$pkg}" || warn "Failed to install $pkg"
                    ;;
                fedora)
                    install_copr_package "${EXTRA_FEDORA[$pkg]:-$pkg}" || warn "Failed to install $pkg"
                    ;;
                ubuntu|debian)
                    warn "$pkg not in repos - see: https://github.com/Open-Wine-Components/umu-launcher"
                    ;;
                *)
                    warn "Don't know how to install $pkg on $DISTRO"
                    ;;
            esac
        done

        info "Dependencies installed"
    else
        echo ""
        echo -e "  ${YELLOW}Manual install:${NC}"
        case "$PKG_MGR" in
            pacman) echo -e "    ${GREEN}sudo pacman -S ${unique_pkgs[*]}${NC}" ;;
            dnf)    echo -e "    ${GREEN}sudo dnf install ${unique_pkgs[*]}${NC}" ;;
            apt)    echo -e "    ${GREEN}sudo apt install ${unique_pkgs[*]}${NC}" ;;
            zypper) echo -e "    ${GREEN}sudo zypper install ${unique_pkgs[*]}${NC}" ;;
        esac
        echo ""
        return 1
    fi
}

# =============================================================================
# Get Cargo Target Directory
# =============================================================================

get_target_dir() {
    if [[ -n "${CARGO_TARGET_DIR:-}" ]]; then
        echo "$CARGO_TARGET_DIR"
    elif [[ -f "$HOME/.cache/cargo/target/release/splitux" ]]; then
        echo "$HOME/.cache/cargo/target"
    else
        echo "$SCRIPT_DIR/target"
    fi
}

# =============================================================================
# Download Dependencies
# =============================================================================

download_goldberg() {
    local gbe_out="$SCRIPT_DIR/res/goldberg"
    local gbe_repo="gabrielgad/gbe_fork-splitux"
    local gbe_release="nightly"

    # Check if already available
    if [[ -f "$gbe_out/linux64/libsteam_api.so" ]] && [[ -f "$gbe_out/win/steam_api64.dll" ]]; then
        info "Goldberg already available"
        copy_steam_client_libs
        return 0
    fi

    step "Downloading Goldberg Steam emulator..."
    local tmp_dir=$(mktemp -d)
    local base_url="https://github.com/$gbe_repo/releases/download/$gbe_release"

    # Download Linux and Windows in parallel
    curl -fsSL "$base_url/emu-linux-release.tar.bz2" -o "$tmp_dir/linux.tar.bz2" &
    local linux_pid=$!
    curl -fsSL "$base_url/emu-win-release.7z" -o "$tmp_dir/win.7z" &
    local win_pid=$!

    # Wait for downloads
    if ! wait $linux_pid; then
        warn "Failed to download Goldberg (linux)"
        rm -rf "$tmp_dir"
        return 1
    fi
    if ! wait $win_pid; then
        warn "Failed to download Goldberg (windows)"
        rm -rf "$tmp_dir"
        return 1
    fi

    # Extract (sequential - both write to release/)
    mkdir -p "$gbe_out"/{linux64,linux32,win,steamnetworkingsockets/{x64,x32}}

    tar -xjf "$tmp_dir/linux.tar.bz2" -C "$tmp_dir"
    cp -f "$tmp_dir"/release/regular/x64/*.so "$gbe_out/linux64/" 2>/dev/null || true
    cp -f "$tmp_dir"/release/regular/x32/*.so "$gbe_out/linux32/" 2>/dev/null || true

    7z x -y -o"$tmp_dir" "$tmp_dir/win.7z" >/dev/null
    cp -f "$tmp_dir"/release/regular/x64/*.dll "$gbe_out/win/" 2>/dev/null || true
    cp -f "$tmp_dir"/release/regular/x32/*.dll "$gbe_out/win/" 2>/dev/null || true
    cp -f "$tmp_dir"/release/steamnetworkingsockets/x64/*.dll "$gbe_out/steamnetworkingsockets/x64/" 2>/dev/null || true
    cp -f "$tmp_dir"/release/steamnetworkingsockets/x32/*.dll "$gbe_out/steamnetworkingsockets/x32/" 2>/dev/null || true

    rm -rf "$tmp_dir"
    info "Goldberg downloaded"
    copy_steam_client_libs
}

copy_steam_client_libs() {
    local gbe_out="$SCRIPT_DIR/res/goldberg"
    local steam_dir="$HOME/.local/share/Steam"

    # 64-bit
    if [[ -f "$steam_dir/linux64/steamclient.so" ]] && [[ ! -f "$gbe_out/linux64/steamclient.so" ]]; then
        cp -f "$steam_dir/linux64/steamclient.so" "$gbe_out/linux64/" 2>/dev/null && \
            info "Copied steamclient.so (64-bit)"
    fi
    [[ -f "$steam_dir/linux64/crashhandler.so" ]] && [[ ! -f "$gbe_out/linux64/crashhandler.so" ]] && \
        cp -f "$steam_dir/linux64/crashhandler.so" "$gbe_out/linux64/" 2>/dev/null

    # 32-bit
    if [[ -f "$steam_dir/linux32/steamclient.so" ]] && [[ ! -f "$gbe_out/linux32/steamclient.so" ]]; then
        cp -f "$steam_dir/linux32/steamclient.so" "$gbe_out/linux32/" 2>/dev/null && \
            info "Copied steamclient.so (32-bit)"
    fi
    [[ -f "$steam_dir/linux32/crashhandler.so" ]] && [[ ! -f "$gbe_out/linux32/crashhandler.so" ]] && \
        cp -f "$steam_dir/linux32/crashhandler.so" "$gbe_out/linux32/" 2>/dev/null || true
}

download_bepinex() {
    local bepinex_out="$SCRIPT_DIR/res/bepinex"
    local need_mono=false
    local need_il2cpp=false

    [[ ! -d "$bepinex_out/mono/core" ]] && need_mono=true
    [[ ! -d "$bepinex_out/il2cpp/core" ]] && need_il2cpp=true

    if [[ "$need_mono" == false ]] && [[ "$need_il2cpp" == false ]]; then
        info "BepInEx already available (mono + il2cpp)"
        return 0
    fi

    step "Downloading BepInEx..."
    local tmp_dir=$(mktemp -d)

    # Download needed files in parallel
    if [[ "$need_mono" == true ]]; then
        curl -fsSL "https://github.com/BepInEx/BepInEx/releases/download/v5.4.23.4/BepInEx_win_x64_5.4.23.4.zip" \
            -o "$tmp_dir/mono.zip" &
        local mono_pid=$!
    fi
    if [[ "$need_il2cpp" == true ]]; then
        curl -fsSL "https://github.com/BepInEx/BepInEx/releases/download/v6.0.0-pre.2/BepInEx-Unity.IL2CPP-win-x64-6.0.0-pre.2.zip" \
            -o "$tmp_dir/il2cpp.zip" &
        local il2cpp_pid=$!
    fi

    # Wait and extract
    if [[ "$need_mono" == true ]]; then
        if wait $mono_pid && [[ -f "$tmp_dir/mono.zip" ]]; then
            unzip -q "$tmp_dir/mono.zip" -d "$tmp_dir/mono"
            chmod -R u+rwX "$tmp_dir/mono"
            mkdir -p "$bepinex_out/mono"
            cp -r "$tmp_dir/mono/BepInEx/core" "$bepinex_out/mono/"
            cp -f "$tmp_dir/mono/winhttp.dll" "$bepinex_out/mono/" 2>/dev/null || true
            cp -f "$tmp_dir/mono/doorstop_config.ini" "$bepinex_out/mono/" 2>/dev/null || true
            info "BepInEx 5 (Mono) downloaded"
        else
            warn "Failed to download BepInEx (mono)"
        fi
    fi

    if [[ "$need_il2cpp" == true ]]; then
        if wait $il2cpp_pid && [[ -f "$tmp_dir/il2cpp.zip" ]]; then
            unzip -q "$tmp_dir/il2cpp.zip" -d "$tmp_dir/il2cpp"
            chmod -R u+rwX "$tmp_dir/il2cpp"
            mkdir -p "$bepinex_out/il2cpp"
            cp -r "$tmp_dir/il2cpp/BepInEx/core" "$bepinex_out/il2cpp/"
            cp -f "$tmp_dir/il2cpp/winhttp.dll" "$bepinex_out/il2cpp/" 2>/dev/null || true
            cp -f "$tmp_dir/il2cpp/doorstop_config.ini" "$bepinex_out/il2cpp/" 2>/dev/null || true
            info "BepInEx 6 (IL2CPP) downloaded"
        else
            warn "Failed to download BepInEx (il2cpp)"
        fi
    fi

    rm -rf "$tmp_dir"
    return 0
}

# =============================================================================
# Build
# =============================================================================

build_gamescope_splitux() {
    local gsc_dir="$SCRIPT_DIR/deps/gamescope"
    local gsc_build="$gsc_dir/build"
    local gsc_bin="$gsc_build/src/gamescope"

    # Check if already built
    if [[ -f "$gsc_bin" ]]; then
        info "gamescope-splitux already built"
        return 0
    fi

    if [[ ! -d "$gsc_dir" ]]; then
        error "gamescope submodule not found. Run: git submodule update --init"
    fi

    step "Building gamescope-splitux (this may take a while)..."
    cd "$gsc_dir"

    # Init gamescope's own submodules (some may have broken refs, init individually)
    step "Initializing gamescope submodules..."
    git submodule update --init src/reshade 2>/dev/null || true
    git submodule update --init thirdparty/SPIRV-Headers 2>/dev/null || true
    git submodule update --init subprojects/wlroots 2>/dev/null || true
    git submodule update --init subprojects/vkroots 2>/dev/null || true
    git submodule update --init subprojects/libliftoff 2>/dev/null || true
    git submodule update --init subprojects/libdisplay-info 2>/dev/null || true
    # Remove broken glm submodule dir if exists (meson will use wrap file)
    [[ -d "subprojects/glm" ]] && rm -rf "subprojects/glm"

    # Configure with meson
    if [[ ! -d "$gsc_build" ]]; then
        meson setup build/ \
            --buildtype=release \
            -Dpipewire=disabled \
            -Ddrm_backend=disabled \
            -Dsdl2_backend=enabled \
            -Denable_openvr_support=false \
            -Dinput_emulation=disabled \
            || error "Meson setup failed"
    fi

    # Build
    ninja -C build/ -j"$(nproc)" || error "Gamescope build failed"

    [[ ! -f "$gsc_bin" ]] && error "gamescope binary not found after build"
    info "gamescope-splitux built"
    cd "$SCRIPT_DIR"
}

build_splitux() {
    step "Building splitux..."
    cd "$SCRIPT_DIR"
    cargo build --release -j"$(nproc)"

    local target_dir=$(get_target_dir)
    [[ ! -f "$target_dir/release/splitux" ]] && error "Binary not found"
    info "splitux built"
}

do_build() {
    check_deps build || exit 1

    # Download dependencies in parallel
    step "Fetching dependencies..."
    download_goldberg &
    local gbe_pid=$!
    download_bepinex &
    local bep_pid=$!

    # Build gamescope-splitux first (takes longest)
    build_gamescope_splitux

    # Build splitux
    build_splitux

    # Wait for downloads (disable errexit for wait)
    set +e
    wait $gbe_pid
    local gbe_status=$?
    wait $bep_pid
    local bep_status=$?
    set -e
    [[ $gbe_status -ne 0 ]] && warn "Goldberg download failed"
    [[ $bep_status -ne 0 ]] && warn "BepInEx download failed"

    # Setup build directory
    step "Setting up build directory..."
    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR/res" "$BUILD_DIR/bin"

    local target_dir=$(get_target_dir)
    cp "$target_dir/release/splitux" "$BUILD_DIR/"
    cp "$SCRIPT_DIR/LICENSE" "$BUILD_DIR/" 2>/dev/null || true
    cp -r "$SCRIPT_DIR/res/"* "$BUILD_DIR/res/" 2>/dev/null || true

    # Copy gamescope-splitux
    if [[ -f "$SCRIPT_DIR/deps/gamescope/build/src/gamescope" ]]; then
        cp "$SCRIPT_DIR/deps/gamescope/build/src/gamescope" "$BUILD_DIR/bin/gamescope-splitux"
        chmod +x "$BUILD_DIR/bin/gamescope-splitux"
        info "gamescope-splitux installed to build/bin/"
    else
        warn "gamescope-splitux not found - input holding support will be unavailable"
    fi

    info "Build complete: $BUILD_DIR/"
}

# =============================================================================
# Run / Install / Update / Clean
# =============================================================================

do_run() {
    [[ ! -f "$BUILD_DIR/splitux" ]] && do_build
    check_deps runtime || exit 1
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
}

do_clean() {
    step "Cleaning..."
    rm -rf "$BUILD_DIR"
    rm -rf "$SCRIPT_DIR/res/goldberg/linux32" "$SCRIPT_DIR/res/goldberg/linux64" "$SCRIPT_DIR/res/goldberg/win"
    rm -rf "$SCRIPT_DIR/res/bepinex"
    rm -rf "$SCRIPT_DIR/deps/gamescope/build"
    cargo clean 2>/dev/null || true
    info "Clean complete"
}

# =============================================================================
# Usage
# =============================================================================

usage() {
    detect_distro
    cat <<EOF
${GREEN}Splitux${NC} - Local co-op split-screen gaming for Linux

${CYAN}Usage:${NC} $0 <command> [options]

${CYAN}Commands:${NC}
    build       Build splitux (downloads dependencies automatically)
    run         Build if needed, then run
    install     Install to ~/.local or specified prefix
    update      Check for updates from remote
    check       Verify dependencies
    clean       Remove build artifacts and downloaded deps

${CYAN}Examples:${NC}
    $0 build                # Build everything
    $0 run                  # Build and run
    $0 install              # Install to ~/.local
    $0 install /usr/local   # System-wide install (needs sudo)

${CYAN}Detected:${NC} $DISTRO ($PKG_MGR)${IMMUTABLE:+ [immutable]}
EOF
}

# =============================================================================
# Main
# =============================================================================

case "${1:-}" in
    build)   do_build ;;
    run)     shift; do_run "$@" ;;
    install) do_install "${2:-}" ;;
    update)  do_update ;;
    check)   check_deps "${2:-runtime}" ;;
    clean)   do_clean ;;
    *)       usage ;;
esac
