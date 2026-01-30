# Niri Layout Testing Guide

Testing the niri window manager integration with 1-4 game instances on a single display.

## Architecture Overview

```
niri (host compositor)
  └── gamescope window (nested compositor, one per instance)
        └── bwrap container
              └── game process
```

Each game instance runs inside its own gamescope window. Splitux's niri integration:
1. Detects gamescope windows via `app_id == "gamescope"`
2. Moves windows to floating mode
3. Sets window size based on layout preset
4. Positions windows at absolute coordinates (via delta calculation)

## Layout Presets

### 2-Player
| Preset ID | Name | Layout |
|-----------|------|--------|
| `2p_horizontal` | Top / Bottom | P1 top half, P2 bottom half |
| `2p_vertical` | Side by Side | P1 left half, P2 right half |

### 3-Player
| Preset ID | Name | Layout |
|-----------|------|--------|
| `3p_t_shape` | T-Shape | P1 full top, P2/P3 bottom halves |
| `3p_inverted_t` | Inverted T | P1/P2 top halves, P3 full bottom |
| `3p_left_main` | Left Main | P1 left half, P2/P3 stacked right |
| `3p_right_main` | Right Main | P1/P2 stacked left, P3 right half |

### 4-Player
| Preset ID | Name | Layout |
|-----------|------|--------|
| `4p_grid` | Grid | 2x2 grid (standard quad) |
| `4p_rows` | Rows | Same as grid, reads L→R, T→B |
| `4p_columns` | Columns | P1/P2 left column, P3/P4 right |
| `4p_main_plus_3` | Main + 3 | P1 left 50%, P2/P3/P4 stacked right thirds |

## Manual Testing

### Prerequisites
- Running niri compositor
- gamescope installed
- glxgears (from mesa-utils) for test windows

### Test Script

```bash
#!/bin/bash
# niri-layout-test.sh - Test niri window positioning

INSTANCE_COUNT=${1:-2}
MONITOR=${2:-DP-1}  # Change to your monitor name

# Get monitor dimensions
MONITOR_INFO=$(niri msg --json outputs | jq -r ".\"$MONITOR\".logical")
MON_X=$(echo $MONITOR_INFO | jq -r '.x')
MON_Y=$(echo $MONITOR_INFO | jq -r '.y')
MON_W=$(echo $MONITOR_INFO | jq -r '.width')
MON_H=$(echo $MONITOR_INFO | jq -r '.height')

echo "Monitor: $MONITOR at ${MON_W}x${MON_H}+${MON_X}+${MON_Y}"

# Kill existing test windows
pkill -f "gamescope.*glxgears" 2>/dev/null
sleep 0.5

# Launch gamescope instances
for i in $(seq 1 $INSTANCE_COUNT); do
    gamescope -W 640 -H 480 -- glxgears &
    echo "Launched instance $i"
    sleep 0.5
done

sleep 2

# Get all gamescope window IDs
WINDOW_IDS=$(niri msg --json windows | jq -r '.[] | select(.app_id == "gamescope") | .id')
echo "Found windows: $WINDOW_IDS"

# Position windows based on count
position_window() {
    local ID=$1
    local X=$2
    local Y=$3
    local W=$4
    local H=$5

    echo "Positioning window $ID: ${W}x${H}+${X}+${Y}"

    # Focus and float
    niri msg action focus-window --id $ID
    niri msg action move-window-to-floating
    sleep 0.05

    # Set size
    niri msg action set-window-width $W
    niri msg action set-window-height $H
    sleep 0.05

    # Get current position and calculate delta
    CURRENT=$(niri msg --json windows | jq -r ".[] | select(.id == $ID) | .layout.tile_pos_in_workspace_view | \"\(.[0]) \(.[1])\"")
    CX=$(echo $CURRENT | cut -d' ' -f1 | cut -d. -f1)
    CY=$(echo $CURRENT | cut -d' ' -f2 | cut -d. -f1)
    DX=$((X - CX))
    DY=$((Y - CY))

    # Move with delta
    X_ARG=$([ $DX -ge 0 ] && echo "+$DX" || echo "$DX")
    Y_ARG=$([ $DY -ge 0 ] && echo "+$DY" || echo "$DY")
    niri msg action move-floating-window --id $ID -x $X_ARG -y $Y_ARG
}

# Apply 2p_vertical (side by side) layout for 2 instances
if [ $INSTANCE_COUNT -eq 2 ]; then
    IDS=($WINDOW_IDS)
    HALF_W=$((MON_W / 2))
    position_window ${IDS[0]} $MON_X $MON_Y $HALF_W $MON_H
    position_window ${IDS[1]} $((MON_X + HALF_W)) $MON_Y $HALF_W $MON_H
fi

# Apply 4p_grid layout for 4 instances
if [ $INSTANCE_COUNT -eq 4 ]; then
    IDS=($WINDOW_IDS)
    HALF_W=$((MON_W / 2))
    HALF_H=$((MON_H / 2))
    position_window ${IDS[0]} $MON_X $MON_Y $HALF_W $HALF_H                    # top-left
    position_window ${IDS[1]} $((MON_X + HALF_W)) $MON_Y $HALF_W $HALF_H       # top-right
    position_window ${IDS[2]} $MON_X $((MON_Y + HALF_H)) $HALF_W $HALF_H       # bottom-left
    position_window ${IDS[3]} $((MON_X + HALF_W)) $((MON_Y + HALF_H)) $HALF_W $HALF_H  # bottom-right
fi

# Verify final positions
echo ""
echo "=== Final Window States ==="
niri msg --json windows | jq '.[] | select(.app_id == "gamescope") | {id, pos: .layout.tile_pos_in_workspace_view, size: .layout.window_size}'

echo ""
echo "Press Enter to clean up..."
read
pkill -f "gamescope.*glxgears"
```

### Expected Results

For a 1920x1080 display at position (0, 0):

**2-player side-by-side (2p_vertical):**
| Window | Position | Size |
|--------|----------|------|
| P1 | (0, 0) | 960x1080 |
| P2 | (960, 0) | 960x1080 |

**4-player grid (4p_grid):**
| Window | Position | Size |
|--------|----------|------|
| P1 | (0, 0) | 960x540 |
| P2 | (960, 0) | 960x540 |
| P3 | (0, 540) | 960x540 |
| P4 | (960, 540) | 960x540 |

## Automated Test via Rust

A proper integration test would use splitux's internal APIs:

```rust
// Pseudocode for integration test
#[test]
fn test_niri_4p_grid_layout() {
    let manager = NiriManager::new();

    // Create mock layout context
    let preset = &PRESET_4P_GRID;
    let ctx = LayoutContext {
        instances: vec![Instance::mock(); 4],
        monitors: vec![Monitor { x: 0, y: 0, width: 1920, height: 1080, .. }],
        preset,
        instance_to_region: vec![0, 1, 2, 3],
    };

    // Launch 4 gamescope windows
    // ...

    // Apply positioning
    manager.on_instances_launched(&ctx).unwrap();

    // Verify each window position
    let windows = manager.get_gamescope_windows().unwrap();
    assert_eq!(windows.len(), 4);

    // Check positions match expected
    // P1: (0,0) P2: (960,0) P3: (0,540) P4: (960,540)
}
```

## Known Limitations

1. **Position jitter**: Small delays between niri actions may cause brief visual glitches
2. **Window decorations**: Some themes add borders that affect actual visible size
3. **Gamescope resize resistance**: Gamescope may enforce minimum sizes based on internal resolution

## Debug Commands

```bash
# List all windows
niri msg --json windows | jq '.[] | {id, app_id, is_floating, pos: .layout.tile_pos_in_workspace_view, size: .layout.window_size}'

# List monitors
niri msg --json outputs | jq 'to_entries[] | {name: .key, x: .value.logical.x, y: .value.logical.y, w: .value.logical.width, h: .value.logical.height}'

# Focus specific window
niri msg action focus-window --id <ID>

# Move to floating
niri msg action move-window-to-floating

# Set size
niri msg action set-window-width 640
niri msg action set-window-height 480

# Move floating (delta-based)
niri msg action move-floating-window -x +100 -y -50
```
