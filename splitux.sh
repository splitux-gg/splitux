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
    local mode="${1:-runtime}"
    local missing_pacman=()
    local missing_aur=()

    step "Checking dependencies..."

    # Required runtime dependencies (always checked)
    command -v fuse-overlayfs >/dev/null 2>&1 || missing_pacman+=("fuse-overlayfs")
    command -v bwrap >/dev/null 2>&1 || missing_pacman+=("bubblewrap")

    # Build dependencies
    if [[ "$mode" == "build" ]]; then
        # Build tools
        command -v cargo >/dev/null 2>&1 || missing_pacman+=("rust")
        command -v meson >/dev/null 2>&1 || missing_pacman+=("meson")
        command -v ninja >/dev/null 2>&1 || missing_pacman+=("ninja")
        command -v cmake >/dev/null 2>&1 || missing_pacman+=("cmake")
        command -v pkg-config >/dev/null 2>&1 || missing_pacman+=("pkgconf")
        command -v git >/dev/null 2>&1 || missing_pacman+=("git")

        # Gamescope build libraries (only if no system gamescope)
        if ! command -v gamescope >/dev/null 2>&1; then
            declare -A pkgmap=(
                ["vulkan"]="vulkan-headers"
                ["libpipewire-0.3"]="pipewire"
                ["wayland-client"]="wayland"
                ["x11"]="libx11"
                ["xkbcommon"]="libxkbcommon"
                ["libdrm"]="libdrm"
                ["libinput"]="libinput"
                ["sdl2"]="sdl2"
                ["xcomposite"]="libxcomposite"
                ["xtst"]="libxtst"
                ["xres"]="libxres"
                ["xmu"]="libxmu"
                ["libcap"]="libcap"
                ["libdecor-0"]="libdecor"
                ["libavif"]="libavif"
                ["benchmark"]="benchmark"
                ["lcms2"]="lcms2"
                ["libdisplay-info"]="libdisplay-info"
                ["pixman-1"]="pixman"
            )

            for pkg in "${!pkgmap[@]}"; do
                if ! pkg-config --exists "$pkg" 2>/dev/null; then
                    missing_pacman+=("${pkgmap[$pkg]}")
                fi
            done

            # Check vulkan header specifically
            if ! echo '#include <vulkan/vulkan.h>' | cpp -x c - >/dev/null 2>&1; then
                [[ ! " ${missing_pacman[*]} " =~ " vulkan-headers " ]] && missing_pacman+=("vulkan-headers")
            fi
        fi
    fi

    # umu-launcher for Windows games
    command -v umu-run >/dev/null 2>&1 || missing_aur+=("umu-launcher")

    # Prompt to install missing dependencies
    if [[ ${#missing_pacman[@]} -gt 0 ]] || [[ ${#missing_aur[@]} -gt 0 ]]; then
        echo ""
        warn "Missing dependencies detected"
        echo ""

        # Remove duplicates
        local unique_pacman=($(printf '%s\n' "${missing_pacman[@]}" | sort -u))
        local unique_aur=($(printf '%s\n' "${missing_aur[@]}" | sort -u))

        if [[ ${#unique_pacman[@]} -gt 0 ]]; then
            echo -e "  ${CYAN}Pacman packages:${NC} ${unique_pacman[*]}"
        fi
        if [[ ${#unique_aur[@]} -gt 0 ]]; then
            echo -e "  ${CYAN}AUR packages:${NC} ${unique_aur[*]}"
        fi
        echo ""

        read -p "  Would you like to install missing dependencies now? [Y/n] " -n 1 -r
        echo ""

        if [[ $REPLY =~ ^[Yy]$ ]] || [[ -z $REPLY ]]; then
            # Install pacman packages
            if [[ ${#unique_pacman[@]} -gt 0 ]]; then
                step "Installing pacman packages..."
                sudo pacman -S --needed "${unique_pacman[@]}" || {
                    error "Failed to install pacman packages"
                }
            fi

            # Install AUR packages
            if [[ ${#unique_aur[@]} -gt 0 ]]; then
                step "Installing AUR packages..."
                if command -v yay >/dev/null 2>&1; then
                    yay -S --needed "${unique_aur[@]}" || {
                        error "Failed to install AUR packages"
                    }
                elif command -v paru >/dev/null 2>&1; then
                    paru -S --needed "${unique_aur[@]}" || {
                        error "Failed to install AUR packages"
                    }
                else
                    echo ""
                    error "No AUR helper found. Install yay or paru, then run: yay -S ${unique_aur[*]}"
                fi
            fi

            info "Dependencies installed successfully"
        else
            echo ""
            echo -e "  ${YELLOW}To install manually:${NC}"
            if [[ ${#unique_pacman[@]} -gt 0 ]]; then
                echo -e "    ${GREEN}sudo pacman -S ${unique_pacman[*]}${NC}"
            fi
            if [[ ${#unique_aur[@]} -gt 0 ]]; then
                echo -e "    ${GREEN}yay -S ${unique_aur[*]}${NC}"
            fi
            echo ""
            exit 1
        fi
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

copy_steam_client_libs() {
    # Copy Steam's steamclient.so to Goldberg directories
    # Games need both libsteam_api.so (from Goldberg) AND steamclient.so (from Steam)
    local gbe_out="$SCRIPT_DIR/res/goldberg"
    local steam_dir="$HOME/.local/share/Steam"

    # Copy 64-bit libraries
    if [[ -f "$steam_dir/linux64/steamclient.so" ]] && [[ ! -f "$gbe_out/linux64/steamclient.so" ]]; then
        cp -f "$steam_dir/linux64/steamclient.so" "$gbe_out/linux64/" 2>/dev/null && \
            info "Copied steamclient.so (64-bit) to goldberg"
    fi
    if [[ -f "$steam_dir/linux64/crashhandler.so" ]] && [[ ! -f "$gbe_out/linux64/crashhandler.so" ]]; then
        cp -f "$steam_dir/linux64/crashhandler.so" "$gbe_out/linux64/" 2>/dev/null
    fi

    # Copy 32-bit libraries
    if [[ -f "$steam_dir/linux32/steamclient.so" ]] && [[ ! -f "$gbe_out/linux32/steamclient.so" ]]; then
        cp -f "$steam_dir/linux32/steamclient.so" "$gbe_out/linux32/" 2>/dev/null && \
            info "Copied steamclient.so (32-bit) to goldberg"
    fi
    if [[ -f "$steam_dir/linux32/crashhandler.so" ]] && [[ ! -f "$gbe_out/linux32/crashhandler.so" ]]; then
        cp -f "$steam_dir/linux32/crashhandler.so" "$gbe_out/linux32/" 2>/dev/null
    fi

    # Create sdk32/sdk64 symlinks if they don't exist (needed by splitux)
    if [[ ! -L "$steam_dir/sdk32" ]]; then
        ln -sf linux32 "$steam_dir/sdk32" 2>/dev/null
    fi
    if [[ ! -L "$steam_dir/sdk64" ]]; then
        ln -sf linux64 "$steam_dir/sdk64" 2>/dev/null
    fi
}

build_bepinex() {
    local bepinex_out="$SCRIPT_DIR/res/bepinex"

    if [[ -d "$bepinex_out/core" ]] && [[ -f "$bepinex_out/winhttp.dll" ]]; then
        info "BepInEx already available"
        return 0
    fi

    step "Downloading BepInEx (Unity IL2CPP)..."
    local tmp_dir
    tmp_dir=$(mktemp -d)

    # Download BepInEx for Unity IL2CPP (Windows x64)
    # This is the most common Unity build type for modern games
    curl -L "https://github.com/BepInEx/BepInEx/releases/download/v6.0.0-pre.2/BepInEx-Unity.IL2CPP-win-x64-6.0.0-pre.2.zip" \
        -o "$tmp_dir/bepinex.zip" || {
        warn "Failed to download BepInEx"
        rm -rf "$tmp_dir"
        return 1
    }

    unzip -q "$tmp_dir/bepinex.zip" -d "$tmp_dir/bepinex" || {
        warn "Failed to extract BepInEx"
        rm -rf "$tmp_dir"
        return 1
    }

    # Fix restrictive permissions from the zip archive
    chmod -R u+rwX "$tmp_dir/bepinex"

    mkdir -p "$bepinex_out"
    cp -r "$tmp_dir/bepinex/BepInEx/core" "$bepinex_out/"
    cp -f "$tmp_dir/bepinex/winhttp.dll" "$bepinex_out/" 2>/dev/null || true
    cp -f "$tmp_dir/bepinex/doorstop_config.ini" "$bepinex_out/" 2>/dev/null || true
    cp -f "$tmp_dir/bepinex/.doorstop_version" "$bepinex_out/" 2>/dev/null || true

    rm -rf "$tmp_dir"
    info "BepInEx downloaded"
}

build_goldberg() {
    local gbe_out="$SCRIPT_DIR/res/goldberg"

    if [[ -d "$gbe_out/linux64" ]] && [[ -d "$gbe_out/win" ]]; then
        info "goldberg already available"
        copy_steam_client_libs
        return 0
    fi

    step "Downloading goldberg Steam emulator..."
    local tmp_dir
    tmp_dir=$(mktemp -d)

    # Download Linux binaries
    curl -L "https://github.com/Detanup01/gbe_fork/releases/latest/download/emu-linux-release.tar.bz2" \
        -o "$tmp_dir/goldberg-linux.tar.bz2" || {
        warn "Failed to download goldberg (linux)"
        rm -rf "$tmp_dir"
        return 1
    }

    tar -xjf "$tmp_dir/goldberg-linux.tar.bz2" -C "$tmp_dir" || {
        warn "Failed to extract goldberg (linux)"
        rm -rf "$tmp_dir"
        return 1
    }

    mkdir -p "$gbe_out/linux64" "$gbe_out/linux32"
    cp -f "$tmp_dir"/release/regular/x64/*.so "$gbe_out/linux64/" 2>/dev/null || true
    cp -f "$tmp_dir"/release/regular/x32/*.so "$gbe_out/linux32/" 2>/dev/null || true
    rm -rf "$tmp_dir/release"

    # Download Windows binaries (needed for Proton/Wine games)
    curl -L "https://github.com/Detanup01/gbe_fork/releases/latest/download/emu-win-release.7z" \
        -o "$tmp_dir/goldberg-win.7z" || {
        warn "Failed to download goldberg (windows)"
        rm -rf "$tmp_dir"
        return 1
    }

    7z x -o"$tmp_dir" "$tmp_dir/goldberg-win.7z" >/dev/null || {
        warn "Failed to extract goldberg (windows)"
        rm -rf "$tmp_dir"
        return 1
    }

    mkdir -p "$gbe_out/win"
    cp -f "$tmp_dir"/release/regular/x64/*.dll "$gbe_out/win/" 2>/dev/null || true
    cp -f "$tmp_dir"/release/regular/x32/*.dll "$gbe_out/win/" 2>/dev/null || true

    rm -rf "$tmp_dir"
    info "goldberg downloaded"
    copy_steam_client_libs
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
    build_bepinex &
    local bep_pid=$!
    build_splitux &
    local spx_pid=$!

    # Wait for all
    wait $gsc_pid || warn "gamescope build failed"
    wait $gbe_pid || warn "goldberg build failed"
    wait $bep_pid || warn "BepInEx download failed"
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
