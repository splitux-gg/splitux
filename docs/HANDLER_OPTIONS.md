# Handler Options Reference

This is the complete reference for the **splitux handler format** — the
`handler.yaml` file that tells splitux how to launch a game for local/LAN co-op.
It documents every field, and every tuning knob exposed by the underlying
emulators (Goldberg/gbe_fork and the splitux EOS LAN emu), so a game that needs
specific tuning has the full menu in one place.

- New to handlers? Start with [Quick Start](#quick-start) and copy
  [`assets/handler_template.yaml`](../assets/handler_template.yaml).
- Looking for a specific knob? Jump to the relevant
  [emulator backend](#multiplayer-backends).
- Adding a brand-new backend? See [Extending this format](#extending-this-format).

> **Schema note.** The canonical form is **nested blocks** (`goldberg:`, `eos:`,
> …), shown throughout this doc. Two older dialects still load for
> back-compat — flat dot-notation (`goldberg.settings.x.txt: "2"`) is expanded
> into nested blocks at load time, and the legacy `backend:` + `goldberg_settings:`
> form is migrated automatically — but new handlers should use nested blocks.

---

## Quick Start

A minimal handler needs just three fields plus a way to find the game:

```yaml
name: My Game          # display name in the launcher
exec: game.exe         # executable, relative to the game root
spec_ver: 3            # handler format version (always 3)
steam_appid: 123456    # Steam app id — auto-locates the install
```

That launches the game with no multiplayer backend. To add co-op, drop in **one**
backend block (see [Multiplayer Backends](#multiplayer-backends)). For example,
a Steam P2P game:

```yaml
name: My Game
exec: game.exe
spec_ver: 3
steam_appid: 123456

goldberg:
  settings:
    force_lobby_type.txt: "2"
    invite_all.txt: ""
```

Save it to `~/.local/share/splitux/handlers/<game>/handler.yaml` and it appears
in the launcher.

---

## Field Reference

### Required

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Display name in the launcher. |
| `exec` | string | Executable path **relative to the game root**. `.exe`/`.bat` ⇒ run under Proton; anything else ⇒ native Linux. |
| `spec_ver` | int | Handler format version. Currently **3**. |

### Game Location (choose one)

| Field | Type | Description |
|-------|------|-------------|
| `steam_appid` | int | Steam app id. Auto-detects the install location and fetches artwork. **Preferred.** |
| `path_gameroot` | string | Absolute path to the game folder. For non-Steam games. |

### Launch

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `args` | string | `""` | Command-line arguments passed to the game. Supports `$PROFILE`, `$WIDTH`, `$HEIGHT`, `$RESOLUTION`, `$INSTANCENUM`, `$INSTANCECOUNT`, `$GAMEDIR`, `$HANDLERDIR`. |
| `env` | string | `""` | Space-separated `KEY=VALUE` pairs set in the game's environment. This is where per-game **EOSLAN_\*** tuning goes (see [EOS](#eos-epic-online-services--splitux-eos-lan-emu)). |
| `proton_path` | string | system default | Proton version, e.g. `"Proton - Experimental"`. Windows games only. |
| `working_dir` | string | `""` | Working directory for the game process (and Goldberg's `GseAppPath`), relative to the mounted game root. When empty, cwd defaults to the exec's parent dir. Set only for launcher-shim games where the real binary lives in a subdir but must run with cwd higher up, e.g. native Frozenbyte titles: `exec: _enchanted_edition_/bin/trine1_linux_32bit` + `working_dir: _enchanted_edition_`. |
| `runtime` | string | `""` | Native Linux runtime: `scout`, `soldier`, or empty. |
| `pause_between_starts` | float | `0` | Seconds to wait between launching each instance. Raise it for heavy games (EOS/UE titles often need 10–15s). |
| `sdl2_override` | enum | `No` | Force a specific SDL2 for old native games with controller issues: `No`, `Srt` (Steam Runtime 32-bit), `Sys` (system). |
| `fullscreen` | bool | `false` | Tell gamescope to fullscreen the game (`-f`) so it fills the whole output and confines the cursor (the pointer can't escape the window edges). Right for single-player / online-co-op (`coop_mode: separate`) games. **Leave OFF for `local-split`**, where each instance is a sub-region of one output and must not fullscreen. (splitux still places the window per the chosen layout; with `fullscreen` the window boots already-fullscreen and the WM keeps it that way.) |

### Co-op Topology

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `coop_mode` | enum | `separate` | `separate` = one game instance **per player**, each joining over the network (online/LAN — Terraria, V Rising, Satisfactory). `local-split` = **one** instance shared by N controllers (couch co-op — Brotato); all splitux-together seats drive the single instance. |

---

## Multiplayer Backends

Backends are **auto-detected by the presence of their block** — include a
`goldberg:` block and the Goldberg backend activates. Multiple backends can
coexist (e.g. `eos:` + `goldberg:` is common: EOS carries co-op, Goldberg is the
Steam-ownership boot shim).

| Backend | Emulator / mechanism | Use for |
|---------|----------------------|---------|
| [`goldberg`](#goldberg-steam-emulation--gbe_fork) | gbe_fork (Steam API emu) | Steam P2P / lobby games |
| [`eos`](#eos-epic-online-services--splitux-eos-lan-emu) | splitux EOS LAN emu | Epic Online Services co-op (UE games, Satisfactory, Palworld) |
| [`photon`](#photon-bepinex-localmultiplayer) | BepInEx + LocalMultiplayer mod | Unity Photon games |
| [`facepunch`](#facepunch-bepinex-steamworks-spoof) | BepInEx Steamworks shim | Unity Facepunch.Steamworks games |
| [`standalone`](#standalone-bepinex--thunderstore) | BepInEx + Thunderstore plugins | Games whose own mods handle multiplayer |

---

### Goldberg (Steam emulation — gbe_fork)

Emulates the Steam API/networking via **gbe_fork**, so Steam P2P and lobby games
play over LAN without real Steam. splitux deploys a per-instance Goldberg config
and writes the standard gbe_fork files automatically (`steam_appid.txt`,
`configs.user.ini`, `configs.main.ini`, `custom_broadcasts.txt`,
`auto_accept_invite.txt`, `auto_send_invite.txt`) with a distinct Steam identity
and `listen_port` per instance.

```yaml
goldberg:
  disable_networking: false   # see below
  networking_sockets: false   # see below
  settings:                   # extra/override gbe_fork settings files
    force_lobby_type.txt: "2"
    invite_all.txt: ""
  plugin:                     # optional BepInEx plugin (see PluginSource)
    source: thunderstore
    community: riskofrain2
    package: someone/GoldbergLocalCoop
    version: "1.0.0"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `disable_networking` | bool | `false` | Sets gbe_fork `disable_networking`. `true` makes Goldberg a pure offline/ownership shim (no Steam networking) — pair with a game's own LAN or with EOS. `false` keeps Steam networking for P2P/lobby discovery. |
| `networking_sockets` | bool | `false` | Also replace `GameNetworkingSockets.dll`. Needed by some games that use the standalone GNS library. |
| `settings` | map | `{}` | Arbitrary gbe_fork settings **files**: each key is a filename written into the Goldberg `settings/` dir, the value is its contents. |
| `plugin` | [PluginSource](#pluginsource) | — | Install BepInEx and fetch a plugin (e.g. a P2P-connection fix). Triggers BepInEx setup. |

**Common `settings` files** (gbe_fork conventions — drop any of these under `settings:`):

| File | Value | Effect |
|------|-------|--------|
| `force_lobby_type.txt` | `0`/`1`/`2` | Lobby visibility: 0 = private, 1 = friends-only, 2 = public. `2` is the usual choice for local discovery. |
| `invite_all.txt` | `""` | Presence of the (empty) file auto-invites all players. |
| `disable_lan_only.txt` | `0`/`1` | Toggle Goldberg's LAN-only mode. |
| `listen_port.txt` | port | Override the per-instance listen port (splitux assigns one automatically by default). |

> Any gbe_fork settings file is valid here — `settings:` is a passthrough.

---

### EOS (Epic Online Services — splitux EOS LAN emu)

Provides Epic's sessions + friends/presence + P2P over LAN via the splitux
**EOS LAN emulator** (`eos_sdk_emu`, a clean-room `EOSSDK-Win64-Shipping.dll`).
This is what UE/Epic co-op games (Satisfactory, Palworld, V Rising) actually ride
on. splitux deploys the emu DLL over the game's bundled EOS SDK at launch.

The emulator is configured **entirely through its native `EOSLAN_*` environment
variables** — there is no JSON config. The `eos:` block enables the backend and
sets a few high-level options; the rest of the tuning is done through the
handler's [`env`](#launch) field.

```yaml
eos:
  appid: "MyGame"                 # informational; identity comes from the username
  enable_lan: true
  disable_online_networking: true

# Per-game EOS emu tuning goes in env (see the EOSLAN_* table below):
env: "EOSLAN_LOCALHOST_MODE=1 EOSLAN_P2P_BASE_PORT=47777"
```

**`eos:` block fields:**

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `appid` | string | `""` | Epic application id. Informational — the emu derives identity from the username, not this. |
| `enable_lan` | bool | `true` | Enable LAN discovery. |
| `disable_online_networking` | bool | `true` | LAN-only; never reach Epic's real servers. |

**EOS emulator tuning — the `EOSLAN_*` environment knobs.** Set these via the
handler's `env:` field. `EOSLAN_USERNAME` is the exception: splitux injects it
**per instance automatically** (= the player's profile name) so each instance
gets a distinct, deterministic Epic identity — do **not** set it in `env`.

| Variable | Set by | Default | Range / values | Purpose |
|----------|--------|---------|----------------|---------|
| `EOSLAN_USERNAME` | **splitux (auto, per-instance)** | `LAN_Player` | string | Display name → deterministic `EpicAccountId`+`ProductUserId`. Must be distinct per instance; splitux sets it to the profile name. UE enforces one-puid↔one-epic, so a shared name breaks the join. |
| `EOSLAN_LOCALHOST_MODE` | handler `env` | `0` (off) | `0`/`1` | Loopback LAN discovery — required when running multiple instances on **one** box. |
| `EOSLAN_P2P_BASE_PORT` | handler `env` | `7777` | 1–65535 | Base P2P port (uses a 99-port range from here). Override to vacate `7777` for games whose own IP/direct-connect netdriver binds it (e.g. Satisfactory's "IP" session type → use `47777`). |
| `EOSLAN_DISCOVERY_PORT` | handler `env` | emu default | 1024–65535 | UDP port for LAN discovery broadcasts. |
| `EOSLAN_BROADCAST_ADDR` | handler `env` | emu default | IP string | Broadcast address for discovery announcements. |
| `EOSLAN_ANNOUNCE_INTERVAL` | handler `env` | emu default | 500–10000 (ms) | How often a session re-announces itself. |
| `EOSLAN_PREFERRED_IP` | handler `env` | auto | IP string | Force the local address the emu binds/advertises (multi-NIC boxes). |
| `EOSLAN_DEBUG` | handler `env` | `0` | `0`/`1` | Enable verbose LAN debug logging. |
| `EOSLAN_LOG_PATH` | handler `env` | `eos-lan.log` | path | Emu log file path. |

> **EOS + Goldberg together.** Steam builds of Epic games still call
> `SteamAPI_Init` at startup for the ownership check. Pair `eos:` with a
> `goldberg: { disable_networking: false }` boot shim so the game starts; co-op
> itself runs over EOS. See the Satisfactory / Palworld / V Rising handlers.

---

### Photon (BepInEx LocalMultiplayer)

For Unity games using Photon networking. splitux injects a `LocalMultiplayer`
BepInEx mod that redirects Photon to a local room, and shares the relevant config
files between instances.

```yaml
photon:
  config_path: "AppData/LocalLow/Company/Game/LocalMultiplayer/global.cfg"
  shared_files:
    - "AppData/LocalLow/Company/Game/LocalMultiplayer/GlobalSave"
  plugin:                       # optional — auto-fetch the mod
    source: thunderstore
    community: repo
    package: Owner/LocalMultiplayer
    version: "1.4.0"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `config_path` | string | `""` | Path (within the profile's windata) to the LocalMultiplayer config file. |
| `shared_files` | list | `[]` | Files shared across all instances (relative to windata). |
| `plugin` | [PluginSource](#pluginsource) | — | Auto-download the mod from Thunderstore. Omit to require manual install (list it under [`required_mods`](#manual-mods-required_mods)). |

---

### Facepunch (BepInEx Steamworks spoof)

For Unity games using `Facepunch.Steamworks`. A BepInEx shim spoofs Steam
identity per instance so each player looks like a distinct, valid Steam user.

```yaml
facepunch:
  spoof_identity: true
  force_valid: true
  photon_bypass: false
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `spoof_identity` | bool | `false` | Give each instance a unique `SteamClient.SteamId`/`Name`. |
| `force_valid` | bool | `false` | Force `SteamClient.IsValid`/`IsLoggedOn` to return true. |
| `photon_bypass` | bool | `false` | Bypass Photon Steam authentication (`AuthType=255`). |

---

### Standalone (BepInEx + Thunderstore)

For games where community mods handle multiplayer themselves (e.g. Dyson Sphere
Program + Nebula). splitux installs BepInEx and the listed plugins from
Thunderstore automatically.

```yaml
standalone:
  community: dyson-sphere-program       # Thunderstore community
  bepinex_package: xiaoye97/BepInEx     # optional; default bbepis/BepInExPack
  plugins:
    - gabrielgad/NebulaMultiplayerMod          # shorthand: latest version
    - package: quackandcheese/LocalMultiplayer  # pinned version
      version: "1.0.4"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `community` | string | `""` | Thunderstore community for plugin downloads. |
| `bepinex_package` | string | `bbepis/BepInExPack` | BepInEx package to install. |
| `plugins` | list of [PluginSource](#pluginsource) | `[]` | Plugins to install. Bare `Owner/Package` strings or full objects. |

---

### PluginSource

Shared shape used by `goldberg.plugin`, `photon.plugin`, and `standalone.plugins`.
A bare `"Owner/Package"` string is accepted as shorthand (latest version,
Thunderstore).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `source` | string | `thunderstore` | `thunderstore`, `github`, or `url`. |
| `community` | string | inherits | Thunderstore community (inherits `standalone.community`). |
| `package` | string | — | `Owner/PackageName`. |
| `version` | string | latest | Pinned version, e.g. `"1.4.0"`. |

---

### Manual mods (`required_mods`)

Document mods the user must install by hand (when there's no auto-fetch source).
Shown in the UI; splitux looks for the files at `dest_path`.

```yaml
required_mods:
  - name: "LocalMultiplayer"
    description: "Steam account spoofing for local co-op"
    url: "https://thunderstore.io/c/peak/p/owner/LocalMultiplayer/"
    dest_path: "overlay/BepInEx/plugins"
    file_pattern: "*.dll"
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | string | Display name (required). |
| `description` | string | What the mod does. |
| `url` | string | Download URL. |
| `dest_path` | string | Where to put it, relative to the handler dir (required). |
| `file_pattern` | string | Expected filename/pattern, e.g. `*.dll`. |

---

## Game Config Patching (`game_patches`)

Rewrite key/value entries in the game's own config files at launch. Auto-detects
set-style (`set key "value"`), ini-style (`key=value`), and space-style
(`key value`).

```yaml
game_patches:
  conf/initial_config_win.cfg:     # path relative to game root
    disable_steam: "1"             # key: value to set
  another/config.ini:
    setting1: value1
```

(Example: The Riftbreaker sets `disable_steam: "1"` to use native LAN instead of
gbe_fork's GameNetworkingSockets path.)

---

## Save Game Integration

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `original_save_path` | string | `""` | Source save data to seed each profile. Supports `~`/`$HOME`, Windows-relative (`AppData/LocalLow/...`, relative to the Wine prefix user dir), and absolute paths. |
| `save_sync_back` | bool | `false` | After the session, sync the first named profile's saves back to the original location (originals are always backed up to `~/.local/share/splitux/save_backups/`). |
| `save_steam_id_remap` | bool | `false` | Rename save files to use each profile's Goldberg Steam ID. |

---

## Advanced

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `game_null_paths` | list | `[]` | Paths inside the game folder to redirect to `/dev/null` (e.g. `logs/`, `crash_dumps/`). |
| `disable_bwrap` | bool | `false` | Skip the bubblewrap sandbox for this game. |
| `disable_input_isolation` | bool | `false` | Don't mask input devices per instance (every instance sees every device). |
| `gptokeyb` | object | — | Gamepad-to-keyboard mapping (see below). |
| `runtime_patches` | list | `[]` | BepInEx runtime method/property patches (see below). |

**`gptokeyb`** — map a controller to keyboard/mouse for games without native pad
support:

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `profile` | string | — | Built-in (`fps`, `mouse_only`, …, from `assets/gptokeyb/{profile}.gptk`) or `custom` (loads `<handler_dir>/gptokeyb.gptk`). |
| `mouse_scale` | int | `512` | Cursor speed multiplier. |
| `mouse_delay` | int | `16` | Mouse update delay in ms (~60fps). |
| `deadzone` | int | `2000` | Analog stick deadzone. |

**`runtime_patches`** — patch managed (C#) methods/properties at runtime via the
BepInEx PatchActions library:

```yaml
runtime_patches:
  - class: SteamManager
    method: Initialize
    action: force_steam_loaded
```

`class` is required; supply exactly one of `method`/`property`. `action` is one
of: `force_true`, `force_false`, `skip`, `force_steam_loaded`,
`fake_auth_ticket`, `photon_auth_none`, `log_call`.

---

## Extending this format

Adding a new emulator/backend keeps to a fixed pattern, so the docs and code stay
in lockstep:

1. **Code**: add a `Backend<Name>Settings` struct in `src/backend/<name>.rs` and
   an `Option<…>` field + `has_<name>()` helper on `Handler`
   (`src/handler.rs`). Detection is "block present ⇒ backend on".
2. **Template**: add a commented block to
   [`assets/handler_template.yaml`](../assets/handler_template.yaml).
3. **Reference**: add a `### <Name>` section under
   [Multiplayer Backends](#multiplayer-backends) with the field table and any
   emulator env knobs.
4. **Registry**: add a row to the backend table in both READMEs.

Each field is documented as a self-contained row, so growing a backend is
append-only.
