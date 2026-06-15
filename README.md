<p align="center">
    <img src="assets/logo.png" width="300" />
</p>

<p align="center"><em>They killed splitscreen. We brought it back.</em></p>

---

<p align="center">
    <img src=".github/assets/launcher.png" width="80%" />
    <br>
    <em>Splitux Launcher - Game selection and configuration</em>
</p>

<p align="center">
    <img src=".github/assets/riftbreaker-splitscreen.png" width="80%" />
    <br>
    <em>The Riftbreaker - Co-op base defense on an alien planet</em>
</p>

<p align="center">
    <img src=".github/assets/gameplay1.png" width="80%" />
    <br>
    <em>Astroneer - Co-op base building on an alien planet</em>
</p>

<p align="center">
    <img src=".github/assets/ato-splitscreen.png" width="80%" />
    <br>
    <em>Across The Obelisk - Two players in the same multiplayer lobby</em>
</p>

## Features

- **Split-screen multiplayer** - Run multiple game instances with automatic window tiling
- **Remote play (splitux-together)** - Stream instances to friends' browsers over WebRTC; they play a seat from anywhere
- **Controller isolation** - Each instance only sees its assigned controllers
- **Keyboard & mouse support** - Per-instance input isolation via custom Gamescope fork
- **Steam artwork integration** - Automatically fetches game icons and banners from your local Steam library
- **Steam & Epic LAN emulation** - Play online-only games locally via the Goldberg (Steam) and EOS LAN (Epic Online Services) emulators
- **Proton support** - Run Windows games through Proton/UMU Launcher
- **Per-player profiles** - Separate saves, settings, and Steam identities per player
- **Headless CLI** - Discover and launch sessions from a script or over SSH, no GUI
- **niri, Hyprland & KDE Plasma** - Native window manager integration

## How It Works

Splitux launches each game instance inside its own containerized environment:

```
┌────────────────────────────────────────────────────────────┐
│ Splitux                                                    │
│ ┌────────────────┐ ┌────────────────┐ ┌────────────────┐   │
│ │ Gamescope      │ │ Gamescope      │ │ Gamescope      │   │
│ │ ┌────────────┐ │ │ ┌────────────┐ │ │ ┌────────────┐ │   │
│ │ │ Bubblewrap │ │ │ │ Bubblewrap │ │ │ │ Bubblewrap │ │   │
│ │ │ ┌────────┐ │ │ │ │ ┌────────┐ │ │ │ │ ┌────────┐ │ │   │
│ │ │ │  Game  │ │ │ │ │ │  Game  │ │ │ │ │ │  Game  │ │ │   │
│ │ │ └────────┘ │ │ │ │ └────────┘ │ │ │ │ └────────┘ │ │   │
│ │ └────────────┘ │ │ └────────────┘ │ │ └────────────┘ │   │
│ │ Player 1       │ │ Player 2       │ │ Player 3       │   │
│ └────────────────┘ └────────────────┘ └────────────────┘   │
└────────────────────────────────────────────────────────────┘
```

Each instance runs in:
- **Gamescope** - Nested Wayland compositor that contains the game window and handles input
- **Bubblewrap** - Lightweight sandbox that masks input devices and mounts profile-specific directories
- **Overlay filesystem** - Injects multiplayer DLLs and per-player configurations

## Supported Backends

| Backend | Use Case |
|---------|----------|
| **Goldberg** | Steam P2P games - emulates Steam networking (gbe_fork) for LAN play |
| **EOS** | Epic Online Services co-op - LAN-emulates Epic sessions/presence/P2P (UE games, Satisfactory, Palworld, V Rising) |
| **Photon** | Unity Photon games - injects LocalMultiplayer mod via BepInEx |
| **Facepunch** | Unity games using Facepunch.Steamworks - spoofs Steam identity via BepInEx |
| **Standalone** | Games whose own community mods handle multiplayer - installs BepInEx + Thunderstore plugins |
| **None** | Games with native LAN support or single-player |

Backends are auto-detected by the presence of their config block. Multiple backends can coexist (e.g., **EOS + Goldberg**, where EOS carries co-op and Goldberg is the Steam-ownership boot shim).

See **[docs/HANDLER_OPTIONS.md](docs/HANDLER_OPTIONS.md)** for every backend's tuning knobs (including the full `EOSLAN_*` emulator environment).

## Installation

### Requirements

- **Window Manager**: niri, Hyprland, or KDE Plasma
- **Dependencies**: Gamescope, Bubblewrap, fuse-overlayfs, SDL2
- **For remote play (splitux-together)**: PipeWire (instance capture) and a working hardware/software H.264 encoder

### GitHub Releases

All builds are available on the [Releases](https://github.com/splitux-gg/splitux/releases) page:

| Release | Description | When to Use |
|---------|-------------|-------------|
| **Latest** | Stable tagged releases (`v1.0.0`, etc.) | Production use |
| **Nightly** | Built daily at 4 AM UTC from `main` | Latest features, may be unstable |

Available formats:
- `.AppImage` - Portable, no installation required
- `.flatpak` - Sandboxed with automatic dependencies
- `.tar.gz` / `.zip` - Native installation

### AppImage (Recommended)

The easiest way to run Splitux. No installation required.

```bash
# Download latest stable
curl -LO https://github.com/splitux-gg/splitux/releases/latest/download/Splitux-x86_64.AppImage

# Or download nightly (latest features, may be unstable)
curl -LO https://github.com/splitux-gg/splitux/releases/download/nightly/Splitux-nightly-x86_64.AppImage

# Make executable and run
chmod +x Splitux-*.AppImage
./Splitux-*.AppImage
```

**Add to app launcher:**
```bash
# Move to a permanent location
mkdir -p ~/.local/bin
mv Splitux-*.AppImage ~/.local/bin/Splitux.AppImage

# Create desktop entry
cat > ~/.local/share/applications/splitux.desktop << 'EOF'
[Desktop Entry]
Name=Splitux
Comment=Local co-op split-screen gaming for Linux
Exec=$HOME/.local/bin/Splitux.AppImage
Icon=splitux
Terminal=false
Type=Application
Categories=Game;
EOF
```

### Flatpak

Sandboxed installation with automatic dependency management.

```bash
# Download latest stable
curl -LO https://github.com/splitux-gg/splitux/releases/latest/download/Splitux.flatpak

# Or download nightly
curl -LO https://github.com/splitux-gg/splitux/releases/download/nightly/Splitux-nightly.flatpak

# Install
flatpak install --user Splitux*.flatpak

# Run
flatpak run gg.splitux.Splitux
```

Flatpak automatically adds Splitux to your app launcher after installation.

### Tarball (Native)

Traditional installation for maximum performance.

```bash
# Download and extract
curl -LO https://github.com/splitux-gg/splitux/releases/latest/download/splitux-linux-x86_64.tar.gz
tar -xzf splitux-linux-x86_64.tar.gz
cd splitux

# Run directly
./splitux

# Or install to ~/.local (adds to app launcher)
./install.sh
```

**Note:** The tarball requires these dependencies to be installed on your system:
- `gamescope` or use the bundled `gamescope-splitux` in `bin/`
- `bubblewrap`
- `fuse-overlayfs`
- `SDL2`

### Building from Source

Requires Rust 1.85+ (for edition 2024), meson, and ninja.

```bash
git clone --recurse-submodules https://github.com/splitux-gg/splitux.git
cd splitux
cargo build --release
```

The binary will be at `target/release/splitux`. Bundled dependencies live in `assets/`; at runtime splitux looks for them under a system install (`/usr/share/splitux`), a user install (`~/.local/share/splitux`), and finally `assets/` next to the binary.

## Configuration

Settings are stored in `~/.local/share/splitux/`:

```
splitux/
├── handlers/           # Game configurations (handler.yaml + assets)
├── profiles/           # Per-player save data and settings
├── prefixes/           # Wine prefixes for Windows games
└── settings.json       # Global configuration
```

### Handler Format

Games are configured via YAML handlers using nested backend blocks. A handler
needs three fields plus a way to find the game, then one backend block:

```yaml
name: Game Name
exec: game.exe
spec_ver: 3
steam_appid: 12345

# Goldberg backend (auto-detected by the block being present)
goldberg:
  settings:
    force_lobby_type.txt: "2"
    invite_all.txt: ""

# Optional launch settings
args: "-windowed"
env: "DXVK_ASYNC=1"
proton_path: "Proton - Experimental"
pause_between_starts: 5.0
```

**Backend examples:**

```yaml
# EOS (Epic Online Services co-op via the EOS LAN emu)
eos:
  appid: "MyGame"
  enable_lan: true
  disable_online_networking: true
env: "EOSLAN_LOCALHOST_MODE=1"   # EOS emu tuning goes through env (EOSLAN_*)

# Photon (Unity games with BepInEx mod)
photon:
  config_path: "AppData/LocalLow/Company/Game/config.cfg"
  shared_files:
    - "AppData/LocalLow/Company/Game/SharedSave"

# Facepunch (Unity games using Facepunch.Steamworks)
facepunch:
  spoof_identity: true
  force_valid: true
  photon_bypass: true
```

Start from [`assets/handler_template.yaml`](assets/handler_template.yaml) and see
**[docs/HANDLER_OPTIONS.md](docs/HANDLER_OPTIONS.md)** for the complete field and
emulator-tuning reference. Or browse and download community handlers from the
in-app handler registry.

> Older handlers written in dot-notation (`goldberg.settings.x.txt: "2"`) and the
> legacy `backend:` form still load — they're expanded/migrated automatically —
> but new handlers should use nested blocks.

## Window Layouts

Each player count has selectable layout presets, chosen per-count in the launcher:

- **2P**: Top / Bottom, Side by Side
- **3P**: Side by Side, Stacked
- **4P**: Grid, Rows, Columns
- **Fullscreen ("Independent")**: every instance gets its own full-resolution
  output instead of a tile — useful for multi-monitor setups and for remote
  (splitux-together) seats.

Window placement is handled natively per WM (niri, Hyprland, KWin). Launched game
processes are contained in a systemd cgroup scope tied to splitux, so a crash or
exit cleans up every child (gamescope, Proton, wine) with it.

## Remote Play (splitux-together)

splitux-together streams launched instances to friends over the internet — no
splitscreen required. When enabled, each instance gets a seat-streamer sidecar
that owns a virtual gamepad/keyboard/mouse and captures that instance's gamescope
output over PipeWire, H.264-encoding it to a remote browser over WebRTC.

splitux shows one invite URL per seat (`{base}/j/{token}`); hand each link to a
friend, who opens it in a browser and auto-joins that seat. Their input drives the
instance exactly like a local controller. Combine with the **Fullscreen** layout
so each remote player gets a full-resolution stream.

## Headless CLI

Discover and launch sessions without the GUI — handy over SSH or from a script. A
plain `splitux` (no subcommand) still opens the GUI.

```bash
# List what's available
splitux list games
splitux list profiles
splitux list inputs

# Launch a session: one --player per seat
splitux launch --game "Satisfactory" \
  --player profile=Gabe,input=together:gamepad \
  --player profile=Ruth,input=together:kbm
```

`--player` takes comma-separated `key=val`: `profile=<name>` (default `Guest`) and
`input=together:gamepad|together:kbm` (remote-seat inputs). The CLI reuses the
exact scan + launch pipeline the GUI drives.

## Controls

The launcher is fully navigable with a gamepad:

| Input | Action |
|-------|--------|
| D-Pad / Left Stick | Navigate |
| A | Select / Confirm |
| B | Back |
| Y | Change Profile |
| X | Edit Handler |
| Start | Launch Game |
| LB / RB | Switch Tabs |
| Right Stick | Scroll |

## License

MIT License - see [LICENSE](LICENSE)

## Acknowledgments

- [PartyDeck](https://github.com/Seezeed7/PartyDeck) - Original inspiration for split-screen gaming on Linux
- [Gamescope](https://github.com/ValveSoftware/gamescope) - Wayland compositor by Valve
- [Goldberg Steam Emulator](https://github.com/Detanup01/gbe_fork) - Steam API emulation
- [UMU Launcher](https://github.com/Open-Wine-Components/umu-launcher) - Proton launcher
- [Nucleus Co-op](https://github.com/SplitScreen-Me/splitscreenme-nucleus) - Split-screen gaming on Windows
