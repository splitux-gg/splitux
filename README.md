<img src=".github/assets/icon.png" align="left" width="100" height="100">

### `Splitux`

Split-screen gaming on Linux

---

<p align="center">
    <img src=".github/assets/launcher.png" width="49%" />
    <img src=".github/assets/gameplay1.png" width="49%" />
</p>

## Features

- **Split-screen multiplayer** - Run multiple game instances with automatic window tiling
- **Controller isolation** - Each instance only sees its assigned controllers
- **Keyboard & mouse support** - Per-instance input isolation via custom Gamescope fork
- **Steam artwork integration** - Automatically fetches game icons and banners from your local Steam library
- **LAN multiplayer emulation** - Play online-only games locally via Goldberg Steam Emulator
- **Proton support** - Run Windows games through Proton/UMU Launcher
- **Per-player profiles** - Separate saves, settings, and Steam identities per player
- **Hyprland & KDE Plasma** - Native window manager integration

## How It Works

Splitux launches each game instance inside its own containerized environment:

```
┌─────────────────────────────────────────────────────────────┐
│  Splitux                                                    │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐      │
│  │  Gamescope   │  │  Gamescope   │  │  Gamescope   │      │
│  │  ┌────────┐  │  │  ┌────────┐  │  │  ┌────────┐  │      │
│  │  │Bubblewrap│ │  │  │Bubblewrap│ │  │  │Bubblewrap│ │   │
│  │  │  Game   │  │  │  │  Game   │  │  │  │  Game   │  │   │
│  │  └────────┘  │  │  └────────┘  │  │  └────────┘  │      │
│  │  Player 1    │  │  Player 2    │  │  Player 3    │      │
│  └──────────────┘  └──────────────┘  └──────────────┘      │
└─────────────────────────────────────────────────────────────┘
```

Each instance runs in:
- **Gamescope** - Nested Wayland compositor that contains the game window and handles input
- **Bubblewrap** - Lightweight sandbox that masks input devices and mounts profile-specific directories
- **Overlay filesystem** - Injects multiplayer DLLs and per-player configurations

## Supported Backends

| Backend | Use Case |
|---------|----------|
| **Goldberg Steam Emulator** | Steam P2P games - emulates Steam networking for LAN play |
| **Photon + BepInEx** | Unity Photon games - injects LocalMultiplayer mod |
| **None** | Games with native LAN support or single-player |

## Installation

### Requirements

- **Window Manager**: Hyprland or KDE Plasma
- **Dependencies**: Gamescope, Bubblewrap, fuse-overlayfs, SDL2

### From Release

Download the latest release from [Releases](https://github.com/gabrielgad/splitux/releases).

### Building from Source

Requires Rust (nightly), meson, and ninja.

```bash
git clone --recurse-submodules https://github.com/gabrielgad/splitux.git
cd splitux
./splitux.sh build
```

The build script automatically:
- Detects your distro (Arch, Fedora, Ubuntu, Debian, openSUSE)
- Checks for required dependencies
- Builds gamescope-splitux (custom fork with input isolation)
- Compiles the Rust application
- Outputs everything to `build/`

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

Games are configured via YAML handlers:

```yaml
name: "Game Name"
exec: "game.exe"
steam_appid: 12345

backend: goldberg        # or: photon, none
use_goldberg: true

# Optional
args: "-windowed"
env: "DXVK_ASYNC=1"
proton_path: "GE-Proton"
```

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
