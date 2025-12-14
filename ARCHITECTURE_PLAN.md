# Splitux Architecture Refactoring Plan

This document captures the complete architecture redesign for modularizing the splitux codebase. Execute this plan incrementally across sessions.

---

## Executive Summary

**Problem**: `handler.rs` (785 lines) and related files mix concerns, reducing maintainability and reusability.

**Solution**: Domain-driven architecture with two orthogonal axes:
- **Platform** (WHERE games come from): Steam, GOG, Epic, Manual
- **Backend** (HOW multiplayer is enabled): Goldberg, Photon, Facepunch

**Key Decisions**:
1. Dot notation for YAML configs (noob-friendly, no indentation issues)
2. Backends auto-detected by presence of config fields (no explicit `backend:` field)
3. Multiple backends can coexist (Goldberg + Photon + Facepunch)
4. Capability-based traits (not lifecycle methods)
5. Platform as explicit abstraction (extensible to GOG, Epic, etc.)
6. Pure functions for config generators (deterministic)
7. `name.rs` + `name/` module convention (not `mod.rs`)

---

## Architecture Overview

### Orthogonal Concerns Matrix

|               | Steam | GOG | Epic | Manual |
|---------------|-------|-----|------|--------|
| **Goldberg**  | ✓     | ✓   | ✓    | ✓      |
| **Photon**    | ✓     | ✓   | ✓    | ✓      |
| **Facepunch** | ✓     | ✓   | ✓    | ✓      |
| **None**      | ✓     | ✓   | ✓    | ✓      |

**Multiple backends can be active simultaneously** - e.g., Goldberg + Facepunch for games that need both Steam emu and Facepunch patches.

### Role Definitions

| Component | Role | 4-Layer? |
|-----------|------|----------|
| **handler.rs** | Configuration data + I/O | No - just `types.rs`, `io.rs` |
| **platform.rs** | Game source abstraction | Yes (per impl) |
| **backend.rs** | Multiplayer domain logic | Yes (per impl) |
| **launch.rs** | Orchestration | Yes |

**Key Insight**: Handler is Data, Launch is Behavior

---

## YAML Configuration Format

### Dot Notation (Preferred)

Backends are auto-detected by presence of their config fields. No explicit `backend:` field needed.

```yaml
name: "Deep Rock Galactic"
exec: "FSD-Win64-Shipping.exe"
author: "Ghost Ship Games"
version: "1.0"

# Platform - where the game comes from
platform: steam
steam_appid: 548430

# Launch settings
runtime: ""
args: ""
env: ""
proton_path: ""

# Goldberg backend - enables LAN multiplayer via Steam emu
# Presence of any goldberg.* field enables this backend
goldberg.disable_networking: true
goldberg.networking_sockets: false
goldberg.settings.force_lobby_type: "2"
goldberg.settings.invite_all: ""

# Photon backend - for Unity games using Photon networking
# Uncomment to enable
# photon.config_path: "AppData/LocalLow/Company/Game/LocalMultiplayer/global.cfg"
# photon.shared_files: ["AppData/LocalLow/Company/Game/SharedSave"]

# Facepunch backend - for games using Facepunch.Steamworks
# Uncomment to enable (can coexist with Goldberg)
# facepunch.spoof_identity: true
# facepunch.force_valid: true
# facepunch.photon_bypass: false

# Runtime patches for game-specific fixes
# runtime_patches:
#   - class: "SteamManager"
#     method: "Awake"
#     action: "skip"

# Required mods that users must install
# required_mods:
#   - name: "LocalMultiplayer"
#     url: "https://..."
#     dest_path: "BepInEx/plugins"
#     file_pattern: "*.dll"

# Advanced settings
disable_bwrap: false
# game_null_paths: ["path/to/null"]
# game_patches:
#   "conf/settings.cfg":
#     "max_players": "4"

# Save sync
# original_save_path: "AppData/LocalLow/Company/Game/Saves"
# save_sync_back: false
# save_steam_id_remap: false
```

### Platform Examples

**Steam:**
```yaml
platform: steam
steam_appid: 548430
```

**GOG (Future):**
```yaml
platform: gog
gog_id: "1639428654"
```

**Epic (Future):**
```yaml
platform: epic
epic_app_name: "Fortnite"
```

**Manual Path:**
```yaml
platform: manual
path_gameroot: "/mnt/games/MyGame"
```

### Backend Examples

**Goldberg only:**
```yaml
goldberg.disable_networking: true
```

**Photon only:**
```yaml
photon.config_path: "AppData/LocalLow/Company/Game/config.cfg"
```

**Goldberg + Facepunch (coexisting):**
```yaml
goldberg.disable_networking: true
goldberg.settings.force_lobby_type: "2"

facepunch.spoof_identity: true
facepunch.force_valid: true
```

---

## Final Directory Structure

```
src/
├── main.rs
│
├── handler.rs                  # Domain API - Handler struct + helpers
├── handler/
│   ├── types.rs                # RequiredMod, SDL2Override
│   └── io.rs                   # YAML I/O, import/export, scan_handlers
│
├── platform.rs                 # Domain API - Platform trait + PlatformConfig enum
├── platform/
│   ├── steam.rs                # Steam platform API
│   ├── steam/
│   │   ├── types.rs            # SteamPlatform struct
│   │   ├── locate.rs           # steamlocate integration (operations)
│   │   └── cache.rs            # Icon/artwork resolution (operations)
│   ├── gog.rs                  # Future: GOG Galaxy
│   ├── epic.rs                 # Future: Epic Games Store
│   └── manual.rs               # Manual path (no platform)
│
├── backend.rs                  # Domain API - Backend trait
├── backend/
│   ├── goldberg.rs             # Goldberg backend API
│   ├── goldberg/
│   │   ├── types.rs            # GoldbergSettings, SteamApiDll
│   │   ├── pure/
│   │   │   ├── bitness.rs      # detect_bitness()
│   │   │   └── config_gen.rs   # generate_account_name(), steam_id()
│   │   ├── operations/
│   │   │   ├── find_dlls.rs    # find_steam_api_dlls()
│   │   │   └── copy_files.rs   # copy_goldberg_dlls()
│   │   └── pipelines/
│   │       └── setup.rs        # create_all_overlays()
│   │
│   ├── photon.rs               # Photon backend API
│   ├── photon/
│   │   ├── types.rs            # PhotonSettings
│   │   ├── pure/
│   │   │   ├── unity.rs        # detect_unity_backend()
│   │   │   └── config_gen.rs   # config generation
│   │   ├── operations/
│   │   │   ├── bepinex.rs      # BepInEx installation
│   │   │   └── symlinks.rs     # shared file symlinks
│   │   └── pipelines/
│   │       └── setup.rs        # setup_all()
│   │
│   ├── facepunch.rs            # Facepunch backend API
│   └── facepunch/
│       ├── types.rs            # FacepunchSettings, RuntimePatch
│       ├── pure/
│       │   └── config_gen.rs   # splitux.cfg generation
│       ├── operations/
│       │   └── bepinex.rs      # BepInEx + plugin installation
│       └── pipelines/
│           └── setup.rs        # setup_all()
│
├── launch.rs                   # Domain API - Orchestration
├── launch/
│   ├── types.rs                # LaunchContext, LaunchConfig
│   ├── pure/
│   │   └── validation.rs       # Validate launch preconditions
│   ├── operations/
│   │   ├── profiles.rs         # setup_profiles
│   │   └── overlays.rs         # fuse_overlayfs_mount
│   └── pipelines/
│       ├── build_cmds.rs       # launch_cmds() -> Vec<Command>
│       └── execute.rs          # launch_game()
│
├── instance.rs                 # Instance management
├── profiles.rs                 # Profile management
├── input.rs                    # Input device management
├── wm/                         # Window manager backends
├── gamescope.rs
├── bwrap.rs
├── game_patches.rs
├── save_sync.rs
├── proton.rs
├── paths.rs
├── util.rs
└── app/                        # UI code
```

---

## Core Trait Definitions

### Platform Trait

```rust
// src/platform.rs

use std::path::PathBuf;
use std::error::Error;

/// Platform trait - represents where a game comes from
pub trait Platform {
    /// Platform name for identification
    fn name(&self) -> &str;

    /// Get the game's root directory path
    fn game_root_path(&self) -> Result<PathBuf, Box<dyn Error>>;

    /// Get icon URI for display (optional)
    fn icon_uri(&self) -> Option<String> { None }

    /// Get logo image URI (optional)
    fn logo_uri(&self) -> Option<String> { None }

    /// Get hero/banner image URI (optional)
    fn hero_uri(&self) -> Option<String> { None }

    /// Get box art URI (optional)
    fn box_art_uri(&self) -> Option<String> { None }

    /// Platform-specific identifier (appid, product id, etc.)
    fn app_identifier(&self) -> Option<String> { None }
}

/// Enum of platform configurations for serde
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "platform")]
pub enum PlatformConfig {
    #[serde(rename = "steam")]
    Steam { steam_appid: u32 },

    #[serde(rename = "gog")]
    Gog { gog_id: String },

    #[serde(rename = "epic")]
    Epic { epic_app_name: String },

    #[serde(rename = "manual")]
    Manual { path_gameroot: String },
}

impl Default for PlatformConfig {
    fn default() -> Self {
        PlatformConfig::Manual { path_gameroot: String::new() }
    }
}

impl PlatformConfig {
    pub fn as_platform(&self) -> Box<dyn Platform> {
        match self {
            PlatformConfig::Steam { steam_appid } => {
                Box::new(steam::SteamPlatform::new(*steam_appid))
            }
            PlatformConfig::Gog { gog_id } => {
                Box::new(gog::GogPlatform::new(gog_id.clone()))
            }
            PlatformConfig::Epic { epic_app_name } => {
                Box::new(epic::EpicPlatform::new(epic_app_name.clone()))
            }
            PlatformConfig::Manual { path_gameroot } => {
                Box::new(manual::ManualPlatform::new(path_gameroot.clone()))
            }
        }
    }
}

mod steam;
mod gog;
mod epic;
mod manual;
```

### Backend Trait

```rust
// src/backend.rs

use std::path::PathBuf;
use std::error::Error;
use crate::handler::Handler;
use crate::instance::Instance;

/// Capability-based trait for multiplayer backends
pub trait Backend {
    /// Backend name for identification
    fn name(&self) -> &str;

    /// Does this backend require filesystem overlays per instance?
    fn requires_overlay(&self) -> bool;

    /// Create overlay directory for a specific instance
    fn create_overlay(
        &self,
        instance: &Instance,
        handler: &Handler,
    ) -> Result<PathBuf, Box<dyn Error>>;

    /// Setup all instances (install mods, configure files, etc.)
    fn setup_all(
        &self,
        instances: &[Instance],
        handler: &Handler,
    ) -> Result<(), Box<dyn Error>>;

    /// Cleanup temporary files after game session
    fn cleanup(&self) -> Result<(), Box<dyn Error>> { Ok(()) }

    /// Returns configuration files that should be validated
    fn required_files(&self, handler: &Handler) -> Vec<PathBuf> { vec![] }
}

mod goldberg;
mod photon;
mod facepunch;

pub use goldberg::{Goldberg, GoldbergSettings};
pub use photon::{Photon, PhotonSettings};
pub use facepunch::{Facepunch, FacepunchSettings, RuntimePatch};
```

### Handler with Auto-Detected Backends

```rust
// src/handler.rs

use crate::backend::{Backend, GoldbergSettings, PhotonSettings, FacepunchSettings, RuntimePatch};
use crate::platform::PlatformConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::collections::HashMap;

mod types;
mod io;

pub use types::{RequiredMod, SDL2Override};
pub use io::{scan_handlers, import_handler};

#[derive(Clone, Serialize, Deserialize)]
pub struct Handler {
    // Context (not serialized)
    #[serde(skip)]
    pub path_handler: PathBuf,

    // Core metadata
    pub name: String,
    pub exec: String,

    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub info: String,

    // Platform configuration (WHERE the game comes from)
    #[serde(flatten)]
    pub platform: PlatformConfig,

    // Launch configuration
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub runtime: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub args: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub env: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub proton_path: String,
    #[serde(default, skip_serializing_if = "is_default_sdl2")]
    pub sdl2_override: SDL2Override,

    // Backends - auto-detected by presence (dot notation in YAML)
    // Multiple backends can coexist
    #[serde(default, skip_serializing_if = "Option::is_none", flatten)]
    pub goldberg: Option<GoldbergSettings>,

    #[serde(default, skip_serializing_if = "Option::is_none", flatten)]
    pub photon: Option<PhotonSettings>,

    #[serde(default, skip_serializing_if = "Option::is_none", flatten)]
    pub facepunch: Option<FacepunchSettings>,

    // Runtime patches (used by Facepunch backend)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_patches: Vec<RuntimePatch>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_between_starts: Option<f64>,

    // Required mods
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_mods: Vec<RequiredMod>,

    // Advanced
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub game_null_paths: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disable_bwrap: bool,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub game_patches: HashMap<String, HashMap<String, String>>,

    // Save sync
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub original_save_path: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub save_sync_back: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub save_steam_id_remap: bool,
}

impl Handler {
    /// Get platform trait object
    pub fn platform(&self) -> Box<dyn crate::platform::Platform> {
        self.platform.as_platform()
    }

    /// Get all enabled backends (auto-detected by presence)
    pub fn backends(&self) -> Vec<Box<dyn Backend>> {
        let mut backends: Vec<Box<dyn Backend>> = vec![];

        if let Some(settings) = &self.goldberg {
            backends.push(Box::new(crate::backend::Goldberg::new(settings.clone())));
        }

        if let Some(settings) = &self.photon {
            backends.push(Box::new(crate::backend::Photon::new(settings.clone())));
        }

        if let Some(settings) = &self.facepunch {
            backends.push(Box::new(crate::backend::Facepunch::new(
                settings.clone(),
                self.runtime_patches.clone(),
            )));
        }

        backends
    }

    /// Check if any backend is enabled
    pub fn has_backend(&self) -> bool {
        self.goldberg.is_some() || self.photon.is_some() || self.facepunch.is_some()
    }

    /// Get game root path (delegates to platform)
    pub fn get_game_rootpath(&self) -> Result<PathBuf, Box<dyn std::error::Error>> {
        self.platform().game_root_path()
    }

    /// Get icon for display
    pub fn icon(&self) -> egui::ImageSource<'_> {
        // Check local icons first
        for ext in &["png", "jpg", "ico"] {
            let local = self.path_handler.join(format!("icon.{}", ext));
            if local.exists() {
                return format!("file://{}", local.display()).into();
            }
        }
        // Try platform icon
        if let Some(uri) = self.platform().icon_uri() {
            return uri.into();
        }
        // Fallback
        egui::include_image!("../res/executable_icon.png")
    }

    // Other display helpers: display(), display_clamp(), win(), etc.
}
```

---

## Launch Flow

```
User clicks "Launch"
       │
       ▼
launch/pipelines/execute.rs::launch_game()
       │
       ├─► handler.platform().game_root_path()
       │   └── Resolves game installation path
       │
       ├─► for backend in handler.backends()
       │       backend.setup_all(instances, handler)
       │   └── Each enabled backend configures itself
       │
       ├─► launch/operations/profiles.rs::setup_profiles()
       │   └── Creates per-instance profiles
       │
       ├─► launch/pipelines/build_cmds.rs::launch_cmds()
       │   └── Builds Command objects for each instance
       │
       └─► Spawn processes + WM positioning
```

---

## Migration Phases

### Phase 1: Setup Infrastructure (Non-Breaking) ✅ COMPLETE
- [x] Create directory structure: `platform/`, `backend/`, `handler/`, `launch/`
- [x] Define Platform trait in `platform.rs`
- [x] Define Backend trait in `backend.rs`
- [x] Define PlatformConfig enum
- [x] Keep existing files AS-IS (renamed to `*_legacy.rs` with re-exports)
- [x] Create stub implementations for all backends (Goldberg, Photon, Facepunch)
- [x] Create stub implementations for platforms (Steam, Manual)

**Validation**: `cargo build` succeeds, all existing functionality works ✅

### Phase 2: Extract Platform (Steam) ✅ COMPLETE
- [x] Create `platform/steam.rs` (full implementation)
- [x] Create `platform/manual.rs` (complete - just returns path)
- [x] Create `platform/steam/` subdirectory structure
- [x] Move Steam cache methods to `platform/steam/cache.rs`
  - `find_cache_file(appid, filename)` - find any cached file
  - `find_icon(appid)` - find icon image
  - `box_art_uri(appid)` - library_600x900.jpg
  - `logo_uri(appid)` - logo.png
  - `hero_uri(appid)` - library_hero.jpg
  - `header_uri(appid)` - library_header.jpg
  - `icon_uri(appid)` - icon as file:// URI
- [x] Move game path logic to `platform/steam/locate.rs`
  - `find_game_path(app_id)` - uses steamlocate
  - `steam_dir()` - get Steam installation path
  - `get_install_dir_name(app_id)` - get app's install dir name
- [x] Implement full Platform trait for SteamPlatform
- [x] Add `PlatformConfig::as_platform()` method
- [x] Add helper methods: `is_steam()`, `steam_appid()`

**Final Structure:**
```
platform/
├── steam.rs           # SteamPlatform + re-exports
├── steam/
│   ├── cache.rs       # Artwork resolution
│   └── locate.rs      # Game path resolution
└── manual.rs          # ManualPlatform
```

**Validation**: `cargo check` passes ✅ (warnings expected - not yet integrated)

### Phase 3: Extract Handler Core ✅ COMPLETE
- [x] Create `handler/types.rs`: RequiredMod, SDL2Override, PhotonSettings, FacepunchSettings, RuntimePatch
- [x] Create `handler/io.rs`: scan_handlers, import_handler, handlers_dir
- [x] Update handler.rs with module declarations and re-exports
- [x] Maintain backward compatibility via `pub use handler_legacy::*`

**Extracted to `handler/types.rs`** (internal, not yet exported):
- `RequiredMod` - mod installation tracking with pattern matching
- `PhotonSettings` - Photon config with is_empty()
- `FacepunchSettings` - Facepunch config with is_default()
- `RuntimePatch` - BepInEx runtime patches
- `SDL2Override` - SDL2 library override enum
- `is_default_sdl2()` - serde helper

**Extracted to `handler/io.rs`** (exported, shadows legacy):
- `scan_handlers()` - scan handlers directory
- `import_handler()` - import .spx package
- `handlers_dir()` - get handlers directory path

**Still in handler_legacy.rs** (for future migration):
- Handler struct definition
- Handler methods: `from_yaml`, `save`, `export`, `remove_handler`
- Display helpers: `icon()`, `display()`, etc.
- Steam cache methods (will move to platform in Phase 2+)

**Final Structure:**
```
handler/
├── types.rs           # Type definitions (internal)
└── io.rs              # I/O functions (exported)
handler.rs             # Module root + re-exports
handler_legacy.rs      # Legacy Handler struct
```

**Validation**: `cargo check` passes ✅

### Phase 4: Migrate Goldberg Backend ✅ COMPLETE
- [x] Create `backend/goldberg.rs` with full implementation
- [x] Create `backend/goldberg/` subdirectory with 4-layer structure
- [x] Create `backend/goldberg/types.rs`: SteamApiDll, SteamDllType, GoldbergConfig
- [x] Extract pure functions to `backend/goldberg/pure/`
  - `bitness.rs`: `detect_bitness(path, filename) -> bool`
- [x] Extract operations to `backend/goldberg/operations/`
  - `find_dlls.rs`: `find_steam_api_dlls(game_dir) -> Vec<SteamApiDll>`
  - `write_settings.rs`: `write_steam_settings(dir, config, handler_settings, disable_networking)`
  - `create_overlay.rs`: `create_instance_overlay(...) -> PathBuf`
- [x] Create `backend/goldberg/pipelines/setup.rs`: `create_all_overlays()`
- [x] Implement full Backend trait for Goldberg struct
- [x] Wire up `create_overlay()` to use the new module functions

**Final Structure:**
```
backend/goldberg/
├── types.rs           # SteamApiDll, SteamDllType, GoldbergConfig
├── pure.rs            # Module root for pure functions
├── pure/
│   └── bitness.rs     # detect_bitness()
├── operations.rs      # Module root for operations
├── operations/
│   ├── find_dlls.rs   # find_steam_api_dlls()
│   ├── write_settings.rs  # write_steam_settings()
│   └── create_overlay.rs  # create_instance_overlay()
├── pipelines.rs       # Module root for pipelines
└── pipelines/
    └── setup.rs       # create_all_overlays()
backend/goldberg.rs    # Goldberg struct + Backend trait impl + re-exports
```

**Re-exports from goldberg.rs:**
- Types: `GoldbergConfig`, `SteamApiDll`, `SteamDllType`
- Operations: `create_instance_overlay`, `find_steam_api_dlls`, `write_steam_settings`
- Pipelines: `create_all_overlays`
- Pure: `detect_bitness`

**Validation**: `cargo check` passes ✅

### Phase 5: Migrate Photon Backend ✅ COMPLETE
- [x] Create `backend/photon.rs` with full implementation
- [x] Create `backend/photon/` subdirectory with 4-layer structure
- [x] Create `backend/photon/types.rs`: UnityBackend, PhotonConfig, PHOTON_BASE_PORT
- [x] Extract pure functions to `backend/photon/pure/`
  - `unity.rs`: `detect_unity_backend(game_dir) -> UnityBackend`
- [x] Extract operations to `backend/photon/operations/`
  - `bepinex.rs`: BepInEx installation and availability checks
  - `overlay.rs`: `create_instance_overlay()`
  - `config.rs`: `generate_instance_config()`, `PhotonAppIds`
  - `symlinks.rs`: `setup_shared_files()`
- [x] Create `backend/photon/pipelines/setup.rs`: `create_all_overlays()`, `generate_all_configs()`
- [x] Implement full Backend trait for Photon struct

**Final Structure:**
```
backend/photon/
├── types.rs           # UnityBackend, PhotonConfig, PHOTON_BASE_PORT
├── pure.rs            # Module root for pure functions
├── pure/
│   └── unity.rs       # detect_unity_backend()
├── operations.rs      # Module root for operations
├── operations/
│   ├── bepinex.rs     # BepInEx installation
│   ├── overlay.rs     # create_instance_overlay()
│   ├── config.rs      # generate_instance_config()
│   └── symlinks.rs    # setup_shared_files()
├── pipelines.rs       # Module root for pipelines
└── pipelines/
    └── setup.rs       # create_all_overlays(), generate_all_configs()
backend/photon.rs      # Photon struct + Backend trait impl + re-exports
```

**Validation**: `cargo check` passes ✅

### Phase 6: Migrate Facepunch Backend ✅ COMPLETE
- [x] Create `backend/facepunch.rs` with full implementation
- [x] Create `backend/facepunch/` subdirectory with 4-layer structure
- [x] Create `backend/facepunch/types.rs`: FacepunchConfig, RuntimePatch
- [x] Extract pure functions to `backend/facepunch/pure/`
  - `config_gen.rs`: `generate_config_content()` for splitux.cfg
- [x] Extract operations to `backend/facepunch/operations/`
  - `bepinex.rs`: BepInEx installation (with mono-linux support)
  - `overlay.rs`: `create_instance_overlay()`
  - `env.rs`: `get_linux_bepinex_env()` for Linux native games
- [x] Create `backend/facepunch/pipelines/setup.rs`: `create_all_overlays()`
- [x] Implement full Backend trait for Facepunch struct

**Final Structure:**
```
backend/facepunch/
├── types.rs           # FacepunchConfig, RuntimePatch
├── pure.rs            # Module root for pure functions
├── pure/
│   └── config_gen.rs  # generate_config_content()
├── operations.rs      # Module root for operations
├── operations/
│   ├── bepinex.rs     # BepInEx installation (mono-linux)
│   ├── overlay.rs     # create_instance_overlay()
│   └── env.rs         # get_linux_bepinex_env()
├── pipelines.rs       # Module root for pipelines
└── pipelines/
    └── setup.rs       # create_all_overlays()
backend/facepunch.rs   # Facepunch struct + Backend trait impl + re-exports
```

**Re-uses from Photon module:**
- `UnityBackend` enum
- `detect_unity_backend()`
- `bepinex_backend_available()`

**Validation**: `cargo check` passes ✅

### Phase 7: Update Handler Backend Fields ✅ COMPLETE
- [x] Replace old backend enum with optional backend settings
- [x] Add migration logic in from_yaml() for old format
- [x] Implement dot notation parsing (custom YAML expander)
- [x] Update UI with checkboxes for backends
- [x] Update backend_legacy.rs to use new optional fields
- [x] Update photon.rs and facepunch.rs to use new optional fields

**New Handler Fields:**
- `goldberg: Option<GoldbergSettings>` - enables Goldberg if Some
- `photon: Option<PhotonSettings>` - enables Photon if Some
- `facepunch: Option<FacepunchSettings>` - enables Facepunch if Some

**Helper Methods Added:**
- `has_goldberg()`, `has_photon()`, `has_facepunch()`, `has_any_backend()`
- `goldberg_ref()`, `photon_ref()`, `facepunch_ref()`
- `enable_goldberg()`, `enable_photon()`, `enable_facepunch()`
- `disable_goldberg()`, `disable_photon()`, `disable_facepunch()`

**Dot Notation Support:**
- `expand_dot_notation()` - expands "goldberg.disable_networking" to nested structure
- Supports both dot notation and nested YAML formats

**Validation**: `cargo build` passes ✅

### Phase 8: Extract Launch Module ✅ COMPLETE
- [x] Create `launch.rs` + `launch/`
- [x] Move SDL constant to `launch/types.rs`
- [x] Extract validation to `launch/pure/validation.rs`
- [x] Extract profile setup to `launch/operations/profiles.rs`
- [x] Extract overlay mounting to `launch/operations/overlays.rs`
- [x] Extract command building to `launch/pipelines/build_cmds.rs`
- [x] Extract execution to `launch/pipelines/execute.rs`

**Final Structure:**
```
launch/
├── types.rs           # SDL_GAMECONTROLLER_IGNORE_DEVICES constant
├── pure.rs            # Module root
├── pure/
│   └── validation.rs  # validate_runtime(), validate_executable()
├── operations.rs      # Module root
├── operations/
│   ├── profiles.rs    # setup_profiles()
│   └── overlays.rs    # fuse_overlayfs_mount_gamedirs()
├── pipelines.rs       # Module root
└── pipelines/
    ├── build_cmds.rs  # launch_cmds(), print_launch_cmds()
    └── execute.rs     # launch_game()
launch.rs              # Module root + re-exports
```

**Validation**: `cargo build` passes ✅

### Phase 9: Cleanup (PARTIAL)
- [ ] Remove deprecated old files (handler_legacy.rs, backend_legacy.rs, launch_legacy.rs)
- [ ] Remove backward-compat shims
- [ ] Update all imports
- [ ] Run clippy, fix warnings
- [ ] Update documentation

**Note**: Phase 9 cleanup is deferred. Legacy files are kept for backward compatibility during testing. Can be completed when confident that new modules work correctly.

**Current State**: 119 warnings (mostly unused imports in new module structure)

---

## Serde Dot Notation Implementation

To support dot notation like `goldberg.disable_networking`, use serde's `flatten` with a prefix wrapper:

```rust
// backend/goldberg/types.rs

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct GoldbergSettings {
    /// goldberg.disable_networking
    #[serde(default, alias = "goldberg.disable_networking")]
    pub disable_networking: bool,

    /// goldberg.networking_sockets
    #[serde(default, alias = "goldberg.networking_sockets")]
    pub networking_sockets: bool,

    /// goldberg.settings.* (nested HashMap)
    #[serde(default, alias = "goldberg.settings")]
    pub settings: HashMap<String, String>,
}
```

Alternative: Use `serde_with` crate for more flexible dot notation handling, or implement custom deserializer.

---

## 4-Layer Pattern Reference

For domains that use the 4-layer pattern:

```
domain.rs               # Public API (trait impl, re-exports)
domain/
├── types.rs            # Data structures (pure)
├── pure/               # Pure functions (no side effects)
│   └── *.rs
├── operations/         # Atomic side effects
│   └── *.rs
└── pipelines/          # Orchestration (compose pure + operations)
    └── *.rs
```

**Layer Rules**:
- **types.rs**: Data structures, no logic
- **pure/**: NO external calls, NO mutations, deterministic
- **operations/**: ONE side effect per function, atomic
- **pipelines/**: Compose operations + pure, handle errors
- **domain.rs**: Public interface, delegates to pipelines

---

## Files to Track

Current large files and their target locations:

| Current File | Lines | Target Location |
|-------------|-------|-----------------|
| `handler.rs` | 785 | `handler.rs` + `handler/` |
| `goldberg.rs` | 344 | `backend/goldberg.rs` + `backend/goldberg/` |
| `photon.rs` | 421 | `backend/photon.rs` + `backend/photon/` |
| `facepunch.rs` | 277 | `backend/facepunch.rs` + `backend/facepunch/` |
| `launch.rs` | 463 | `launch.rs` + `launch/` |
| `pages_games.rs` | 506 | Consider splitting if needed |

---

## Notes

- Execute phases incrementally - each phase should result in working code
- Run `cargo build` and test after each phase
- Keep backward compatibility for YAML files during migration
- Use re-exports to minimize import changes across codebase
- ~500 lines per file is the target, not a hard rule
- Dot notation makes configs noob-friendly (no indentation management)
- Multiple backends can coexist - iterate over handler.backends()
