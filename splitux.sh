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
    [slirp4netns]="slirp4netns"
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
    [slirp4netns]="slirp4netns"
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
    [slirp4netns]="slirp4netns"
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
    [slirp4netns]="slirp4netns"
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
    local runtime_deps=(fuse-overlayfs bubblewrap slirp4netns)
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

download_gamescope() {
    local gsc_out="$SCRIPT_DIR/res/gamescope-splitux"
    local gsc_repo="splitux-gg/gamescope-splitux"
    local gsc_release
    local local_version=""

    # Fetch latest release from GitHub
    gsc_release=$(curl -fsSL "https://api.github.com/repos/$gsc_repo/releases/latest" | grep -oP '"tag_name":\s*"\K[^"]+')

    if [[ -z "$gsc_release" ]]; then
        warn "Failed to fetch latest gamescope-splitux release, using fallback v1.0.3"
        gsc_release="v1.0.3"
    fi

    # Check local version
    if [[ -f "$gsc_out/.version" ]]; then
        local_version=$(cat "$gsc_out/.version")
    fi

    # Skip if already have latest version
    if [[ -f "$gsc_out/bin/gamescope" ]] && [[ "$local_version" == "$gsc_release" ]]; then
        info "gamescope-splitux $gsc_release already installed"
        return 0
    fi

    if [[ -n "$local_version" ]]; then
        step "Updating gamescope-splitux $local_version → $gsc_release..."
    else
        step "Downloading gamescope-splitux $gsc_release..."
    fi

    local tmp_dir=$(mktemp -d)
    local base_url="https://github.com/$gsc_repo/releases/download/$gsc_release"

    if ! curl -fsSL "$base_url/gamescope-splitux-$gsc_release.zip" -o "$tmp_dir/gamescope.zip"; then
        warn "Failed to download gamescope-splitux"
        rm -rf "$tmp_dir"
        return 1
    fi

    # Extract (remove old first)
    rm -rf "$gsc_out"
    mkdir -p "$gsc_out"
    unzip -q "$tmp_dir/gamescope.zip" -d "$gsc_out"
    chmod +x "$gsc_out/bin/"*

    # Store version
    echo "$gsc_release" > "$gsc_out/.version"

    rm -rf "$tmp_dir"
    info "gamescope-splitux $gsc_release installed"
}

download_gptokeyb() {
    local gptk_out="$SCRIPT_DIR/res/gptokeyb"
    local gptk_repo="splitux-gg/gptokeyb-splitux"
    local gptk_release
    local local_version=""

    # Fetch latest release from GitHub
    gptk_release=$(curl -fsSL "https://api.github.com/repos/$gptk_repo/releases/latest" | grep -oP '"tag_name":\s*"\K[^"]+')

    if [[ -z "$gptk_release" ]]; then
        warn "Failed to fetch latest gptokeyb-splitux release, using fallback v1.0.1"
        gptk_release="v1.0.1"
    fi

    # Check local version
    if [[ -f "$gptk_out/.version" ]]; then
        local_version=$(cat "$gptk_out/.version")
    fi

    # Skip if already have latest version
    if [[ -f "$gptk_out/bin/gptokeyb" ]] && [[ "$local_version" == "$gptk_release" ]]; then
        info "gptokeyb-splitux $gptk_release already installed"
        return 0
    fi

    if [[ -n "$local_version" ]]; then
        step "Updating gptokeyb-splitux $local_version → $gptk_release..."
    else
        step "Downloading gptokeyb-splitux $gptk_release..."
    fi

    local tmp_dir=$(mktemp -d)
    local base_url="https://github.com/$gptk_repo/releases/download/$gptk_release"

    if ! curl -fsSL "$base_url/gptokeyb-splitux-$gptk_release.tar.gz" -o "$tmp_dir/gptokeyb.tar.gz"; then
        warn "Failed to download gptokeyb-splitux"
        rm -rf "$tmp_dir"
        return 1
    fi

    # Extract to temp first, then move files to proper location (remove old first)
    rm -rf "$gptk_out/bin"
    mkdir -p "$gptk_out/bin"
    tar -xzf "$tmp_dir/gptokeyb.tar.gz" -C "$tmp_dir/"
    # Rename gptokeyb2 to gptokeyb for compatibility and move to bin/
    [[ -f "$tmp_dir/gptokeyb2" ]] && mv "$tmp_dir/gptokeyb2" "$gptk_out/bin/gptokeyb"
    # Copy interpose library if present
    [[ -f "$tmp_dir/lib/libinterpose.so" ]] && cp "$tmp_dir/lib/libinterpose.so" "$gptk_out/bin/"
    chmod +x "$gptk_out/bin/"*

    # Store version
    echo "$gptk_release" > "$gptk_out/.version"

    rm -rf "$tmp_dir"
    info "gptokeyb-splitux $gptk_release installed"
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
    local need_mono_win=false
    local need_mono_linux=false
    local need_il2cpp=false

    [[ ! -d "$bepinex_out/mono/core" ]] && need_mono_win=true
    [[ ! -d "$bepinex_out/mono-linux/core" ]] && need_mono_linux=true
    [[ ! -d "$bepinex_out/il2cpp/core" ]] && need_il2cpp=true

    if [[ "$need_mono_win" == false ]] && [[ "$need_mono_linux" == false ]] && [[ "$need_il2cpp" == false ]]; then
        info "BepInEx already available (mono-win + mono-linux + il2cpp)"
        return 0
    fi

    step "Downloading BepInEx..."
    local tmp_dir=$(mktemp -d)
    local bep_ver="5.4.23.4"

    # Download needed files in parallel
    if [[ "$need_mono_win" == true ]]; then
        curl -fsSL "https://github.com/BepInEx/BepInEx/releases/download/v${bep_ver}/BepInEx_win_x64_${bep_ver}.zip" \
            -o "$tmp_dir/mono_win.zip" &
        local mono_win_pid=$!
    fi
    if [[ "$need_mono_linux" == true ]]; then
        curl -fsSL "https://github.com/BepInEx/BepInEx/releases/download/v${bep_ver}/BepInEx_linux_x64_${bep_ver}.zip" \
            -o "$tmp_dir/mono_linux.zip" &
        local mono_linux_pid=$!
    fi
    if [[ "$need_il2cpp" == true ]]; then
        curl -fsSL "https://github.com/BepInEx/BepInEx/releases/download/v6.0.0-pre.2/BepInEx-Unity.IL2CPP-win-x64-6.0.0-pre.2.zip" \
            -o "$tmp_dir/il2cpp.zip" &
        local il2cpp_pid=$!
    fi

    # Wait and extract - Windows Mono
    if [[ "$need_mono_win" == true ]]; then
        if wait $mono_win_pid && [[ -f "$tmp_dir/mono_win.zip" ]]; then
            unzip -q "$tmp_dir/mono_win.zip" -d "$tmp_dir/mono_win"
            chmod -R u+rwX "$tmp_dir/mono_win"
            mkdir -p "$bepinex_out/mono"
            cp -r "$tmp_dir/mono_win/BepInEx/core" "$bepinex_out/mono/"
            cp -f "$tmp_dir/mono_win/winhttp.dll" "$bepinex_out/mono/" 2>/dev/null || true
            cp -f "$tmp_dir/mono_win/doorstop_config.ini" "$bepinex_out/mono/" 2>/dev/null || true
            info "BepInEx 5 (Mono/Windows) downloaded"
        else
            warn "Failed to download BepInEx (mono-win)"
        fi
    fi

    # Wait and extract - Linux Mono
    if [[ "$need_mono_linux" == true ]]; then
        if wait $mono_linux_pid && [[ -f "$tmp_dir/mono_linux.zip" ]]; then
            unzip -q "$tmp_dir/mono_linux.zip" -d "$tmp_dir/mono_linux"
            chmod -R u+rwX "$tmp_dir/mono_linux"
            mkdir -p "$bepinex_out/mono-linux"
            cp -r "$tmp_dir/mono_linux/BepInEx/core" "$bepinex_out/mono-linux/"
            # Linux uses libdoorstop.so and run script instead of winhttp.dll
            cp -f "$tmp_dir/mono_linux/libdoorstop.so" "$bepinex_out/mono-linux/" 2>/dev/null || true
            cp -f "$tmp_dir/mono_linux/run_bepinex.sh" "$bepinex_out/mono-linux/" 2>/dev/null || true
            cp -f "$tmp_dir/mono_linux/.doorstop_version" "$bepinex_out/mono-linux/" 2>/dev/null || true
            chmod +x "$bepinex_out/mono-linux/run_bepinex.sh" 2>/dev/null || true
            chmod +x "$bepinex_out/mono-linux/libdoorstop.so" 2>/dev/null || true
            info "BepInEx 5 (Mono/Linux) downloaded"
        else
            warn "Failed to download BepInEx (mono-linux)"
        fi
    fi

    # Wait and extract - IL2CPP (Windows only for now)
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

download_facepunch() {
    local fp_out="$SCRIPT_DIR/res/facepunch"
    local fp_repo="splitux-gg/bepinex-facepunch-splitux"
    local fp_release="v2.1.0"

    if [[ -f "$fp_out/SplituxFacepunch.dll" ]]; then
        info "SplituxFacepunch already available"
        return 0
    fi

    step "Downloading SplituxFacepunch..."
    mkdir -p "$fp_out"
    local base_url="https://github.com/$fp_repo/releases/download/$fp_release"

    if ! curl -fsSL "$base_url/SplituxFacepunch.dll" -o "$fp_out/SplituxFacepunch.dll"; then
        warn "Failed to download SplituxFacepunch"
        return 1
    fi

    info "SplituxFacepunch downloaded"
}

# =============================================================================
# Build
# =============================================================================

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
    download_gamescope &
    local gsc_pid=$!
    download_gptokeyb &
    local gptk_pid=$!
    download_facepunch &
    local fp_pid=$!

    # Build splitux while dependencies download
    build_splitux

    # Wait for downloads to complete
    wait $gbe_pid 2>/dev/null || warn "Goldberg download may have failed"
    wait $bep_pid 2>/dev/null || warn "BepInEx download may have failed"
    wait $gsc_pid 2>/dev/null || warn "gamescope-splitux download may have failed"
    wait $gptk_pid 2>/dev/null || warn "gptokeyb-splitux download may have failed"
    wait $fp_pid 2>/dev/null || warn "SplituxFacepunch download may have failed"

    # Setup build directory
    step "Setting up build directory..."
    rm -rf "$BUILD_DIR"
    mkdir -p "$BUILD_DIR/res" "$BUILD_DIR/bin"

    local target_dir=$(get_target_dir)
    cp "$target_dir/release/splitux" "$BUILD_DIR/"
    cp "$SCRIPT_DIR/LICENSE" "$BUILD_DIR/" 2>/dev/null || true
    cp -r "$SCRIPT_DIR/res/"* "$BUILD_DIR/res/" 2>/dev/null || true

    # Copy gamescope-splitux from downloaded binaries (rename to gamescope-splitux)
    if [[ -f "$SCRIPT_DIR/res/gamescope-splitux/bin/gamescope" ]]; then
        cp "$SCRIPT_DIR/res/gamescope-splitux/bin/gamescope" "$BUILD_DIR/bin/gamescope-splitux"
        cp "$SCRIPT_DIR/res/gamescope-splitux/bin/gamescopectl" "$BUILD_DIR/bin/" 2>/dev/null || true
        cp "$SCRIPT_DIR/res/gamescope-splitux/bin/gamescopereaper" "$BUILD_DIR/bin/" 2>/dev/null || true
        chmod +x "$BUILD_DIR/bin/"*
        info "gamescope-splitux installed to build/bin/"
    else
        warn "gamescope-splitux not found - input holding support will be unavailable"
    fi

    # Copy gptokeyb from downloaded binaries
    if [[ -f "$SCRIPT_DIR/res/gptokeyb/bin/gptokeyb" ]]; then
        cp "$SCRIPT_DIR/res/gptokeyb/bin/gptokeyb" "$BUILD_DIR/bin/"
        # Copy interpose library if present
        cp "$SCRIPT_DIR/res/gptokeyb/bin/libinterpose"*.so "$BUILD_DIR/bin/" 2>/dev/null || true
        chmod +x "$BUILD_DIR/bin/"*
        info "gptokeyb installed to build/bin/"
    else
        warn "gptokeyb not found - controller-to-keyboard support will be unavailable"
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
    rm -rf "$SCRIPT_DIR/res/bepinex/mono" "$SCRIPT_DIR/res/bepinex/mono-linux" "$SCRIPT_DIR/res/bepinex/il2cpp"
    rm -rf "$SCRIPT_DIR/res/gamescope-splitux"
    rm -rf "$SCRIPT_DIR/res/gptokeyb/bin" "$SCRIPT_DIR/res/gptokeyb/.version"
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
