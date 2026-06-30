# Niri Layout Testing Guide

How splitux places gamescope windows on [niri](https://github.com/YaLTeR/niri), and
how to exercise the layout code by hand. Covers 1–4 instances on one display plus
multi-monitor and same-display-split cases.

## Architecture Overview

```
niri (host compositor)
  └── gamescope window (nested compositor, one per instance)
        └── bwrap container
              └── game process
```

Each game instance is its own gamescope window. splitux's niri integration
(`src/wm/niri.rs`) drives placement entirely through **niri's IPC** (`niri msg`),
using niri's native **tiling columns and fullscreen state** — *not* floating
windows or absolute coordinates.

1. **Find this launch's windows.** A window is ours when its `app_id` contains
   `gamescope` **or** its PID's exe/comm is a gamescope binary (Proton titles on
   niri surface with an unset `app_id`). Results are filtered to the launch's own
   systemd scope cgroup (`splitux-<launch_ns>`), so two concurrent splitux
   sessions never grab each other's windows.
2. **Assign each window to an output** by the instance's chosen monitor
   (`move-window-to-monitor`).
3. **Tile or fullscreen** per the layout:
   - **Tiled presets** (`vertical`/`horizontal`/`grid`): ensure each window is
     tiled (`move-window-to-tiling`), then build columns with `set-column-width`
     and `consume-window-into-column`.
   - **Fullscreen preset / a lone window on an output**: put it into niri
     fullscreen state (`fullscreen-window`), edge-to-edge at full resolution.

### Fullscreen is idempotent (important)

`fullscreen-window` is a **toggle**, and niri exposes no fullscreen-state field.
gamescope's `-f` (the handler [`fullscreen`](HANDLER_OPTIONS.md) flag) boots the
surface *already fullscreen*, so a blind toggle would flip it back to a tiled
column. splitux instead reads the geometry: a fullscreen window's `window_size`
equals the output's logical size **exactly** (a tiled full-width column is
slightly smaller — gaps/border, e.g. 1894×1054 vs 1920×1080). It toggles only
when the window isn't already covering the output. This is the only reliable
fullscreen tell on niri.

## Layout Presets

These are the presets that actually exist (`src/wm/presets.rs`); the friendly
names are what the launcher shows.

### 2-Player
| Preset ID | Name | Layout |
|-----------|------|--------|
| `2p_horizontal` | Top / Bottom | One column, P1 stacked over P2 |
| `2p_vertical` | Side by Side | Two columns, P1 left / P2 right |
| `2p_fullscreen` | Fullscreen | Each its own full-resolution output |

### 3-Player
| Preset ID | Name | Layout |
|-----------|------|--------|
| `3p_vertical` | Side by Side | Three equal columns |
| `3p_horizontal` | Stacked | One column, three stacked |
| `3p_fullscreen` | Fullscreen | Each its own full-resolution output |

### 4-Player
| Preset ID | Name | Layout |
|-----------|------|--------|
| `4p_grid` | Grid | 2×2 — two columns, two stacked each |
| `4p_rows` | Rows | 2×2, read L→R, T→B |
| `4p_columns` | Columns | P1/P2 left column, P3/P4 right |
| `4p_fullscreen` | Fullscreen | Each its own full-resolution output |

## Multi-monitor & same-display split

- **One instance per output** → that instance is fullscreened on its output.
- **Several instances, each on a distinct output** → each fullscreened on its own.
- **More instances than outputs** → instances that must share an output are
  **tiled side-by-side** on it (via a default split for the count: 2→side-by-side,
  3→three columns, 4→2×2 grid), so every player stays visible instead of
  fullscreen-stacking (where niri shows only the focused window). See
  `position_windows_fullscreen` + `default_split_preset`. Tiled slots are
  smaller than the full surface, so gamescope downscales — acceptable, not crisp.

## Manual Testing

### Prerequisites
- Running niri compositor
- gamescope installed (or splitux's bundled `gamescope-splitux`)
- `glxgears` (mesa-utils / mesa-demos) for cheap test clients

### Quick layout iteration without launching a real game

A bare gamescope+glxgears window is enough to iterate placement (it dies without
a graphical client, so keep glxgears as the child):

```bash
# Bring up N cheap gamescope windows, then inspect what niri sees
N=${1:-2}
for i in $(seq 1 "$N"); do
  gamescope -W 1920 -H 1080 -- glxgears &
  sleep 0.5
done
sleep 2

# What splitux's window scan keys off (app_id contains gamescope, or PID is gamescope)
niri msg --json windows | jq -r '.[] | select((.app_id // "") | ascii_downcase | contains("gamescope")) | {id, app_id, size: .layout.window_size}'
```

Then drive niri the way splitux does, to confirm a placement by hand:

```bash
ID=<window-id>
niri msg action focus-window --id "$ID"
niri msg action move-window-to-monitor DP-1     # assign output
niri msg action move-window-to-tiling           # ensure tiled (for split presets)
niri msg action set-column-width 50%            # e.g. one half of a 2p_vertical
# …or, for fullscreen — TOGGLE, only if not already covering the output:
niri msg action fullscreen-window --id "$ID"
```

> `fullscreen-window` is a toggle. Before calling it, check geometry:
> `niri msg --json windows | jq '.[] | select(.id==<ID>) | .layout.window_size'`
> equal to the output's logical size means it's already fullscreen — don't toggle.

### End-to-end through splitux

The real path is exercised by launching a session (CLI is easiest for scripted
iteration):

```bash
splitux launch --game <Game> \
  --player profile=<P>,input=local:gamepad \
  --player profile=<Q>,input=local:gamepad \
  --layout vertical            # or fullscreen / horizontal / grid
# multi-monitor: add --display DP-1 --display HDMI-A-1
```

splitux logs each placement decision (`[splitux] wm::niri - …`), including which
output each window went to and whether it left an already-fullscreen window
alone — the quickest way to confirm the layout code did what you expected.

## Expected geometry (1920×1080 output at 0,0)

**2-player side-by-side (`2p_vertical`)** — two tiled columns:
| Window | Column width | On-screen size (approx, minus gaps) |
|--------|--------------|-------------------------------------|
| P1 | 50% | ~947×1054 |
| P2 | 50% | ~947×1054 |

**4-player grid (`4p_grid`)** — two 50% columns, two stacked each:
| Window | Column | Slot |
|--------|--------|------|
| P1 | left | top |
| P2 | left | bottom |
| P3 | right | top |
| P4 | right | bottom |

**Fullscreen (`*_fullscreen`)** — each window's `window_size` equals the output's
logical size exactly (1920×1080), edge-to-edge with no gaps.
