#!/bin/bash
# Test niri window positioning with multiple gamescope instances
#
# Usage:
#   ./test-niri-layout.sh [OPTIONS] [instance_count]
#
# Options:
#   -m, --monitor NAME    Target monitor (default: HDMI-A-1)
#   -l, --layout PRESET   Layout preset (see below)
#   -f, --floating        Use floating mode instead of tiled
#   -h, --help            Show this help
#
# Layout Presets:
#   vertical   - Side-by-side columns (equal width)
#   horizontal - Stacked rows (equal height)
#   grid       - 2x2 for 4 players
#
# All layouts use EQUAL splits (no biased "main" player):
#   2p: 50%/50%    3p: 33%/33%/33%    4p: 25%/25%/25%/25% or 2x2 grid
#
# Examples:
#   ./test-niri-layout.sh 2                    # 2 windows, vertical (50%/50%)
#   ./test-niri-layout.sh 3                    # 3 windows, vertical (33%/33%/33%)
#   ./test-niri-layout.sh 3 -l horizontal      # 3 windows, stacked rows
#   ./test-niri-layout.sh 4                    # 4 windows, 2x2 grid
#   ./test-niri-layout.sh 4 -l vertical        # 4 windows, 4 columns (25% each)
#   ./test-niri-layout.sh -f 4                 # floating mode

set -e

# Defaults
MONITOR="HDMI-A-1"
MODE="tiled"
INSTANCE_COUNT=2
LAYOUT=""

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -m|--monitor)
            MONITOR="$2"
            shift 2
            ;;
        -l|--layout)
            LAYOUT="$2"
            shift 2
            ;;
        -f|--floating)
            MODE="floating"
            shift
            ;;
        -h|--help)
            head -30 "$0" | tail -28
            exit 0
            ;;
        [0-9]*)
            INSTANCE_COUNT=$1
            shift
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Set default layout based on instance count
if [ -z "$LAYOUT" ]; then
    case $INSTANCE_COUNT in
        1) LAYOUT="fullscreen" ;;
        2|3) LAYOUT="vertical" ;;
        4) LAYOUT="grid" ;;
    esac
fi

echo "=== Niri Layout Test ==="
echo "Monitor:   $MONITOR"
echo "Instances: $INSTANCE_COUNT"
echo "Layout:    $LAYOUT"
echo "Mode:      $MODE"
echo ""

# Get monitor dimensions
MONITOR_INFO=$(niri msg --json outputs | jq -r ".\"$MONITOR\".logical")
if [ "$MONITOR_INFO" = "null" ]; then
    echo "Error: Monitor $MONITOR not found or not active"
    echo ""
    echo "Available monitors:"
    niri msg --json outputs | jq -r 'to_entries[] | select(.value.logical != null) | "  \(.key): \(.value.logical.width)x\(.value.logical.height)+\(.value.logical.x)+\(.value.logical.y)"'
    exit 1
fi

MON_X=$(echo $MONITOR_INFO | jq -r '.x')
MON_Y=$(echo $MONITOR_INFO | jq -r '.y')
MON_W=$(echo $MONITOR_INFO | jq -r '.width')
MON_H=$(echo $MONITOR_INFO | jq -r '.height')

echo "Monitor geometry: ${MON_W}x${MON_H}+${MON_X}+${MON_Y}"
echo ""

# Clean up any existing test windows
echo "Cleaning up existing test windows..."
pkill -f "gamescope.*glxgears" 2>/dev/null || true
sleep 0.5

# Launch gamescope instances
echo "Launching $INSTANCE_COUNT gamescope instances..."
for i in $(seq 1 $INSTANCE_COUNT); do
    gamescope -W 640 -H 480 -- glxgears &
    echo "  Instance $i launched"
    sleep 0.3
done

# Wait for windows to appear
echo "Waiting for windows..."
sleep 2

# Get gamescope window IDs (sorted for consistent ordering)
WINDOW_IDS=($(niri msg --json windows | jq -r '.[] | select(.app_id == "gamescope") | .id' | sort -n))
FOUND_COUNT=${#WINDOW_IDS[@]}

echo "Found $FOUND_COUNT gamescope windows: ${WINDOW_IDS[*]}"

if [ $FOUND_COUNT -lt $INSTANCE_COUNT ]; then
    echo "Warning: Expected $INSTANCE_COUNT windows, found $FOUND_COUNT"
fi

# Move all windows to target monitor first
echo ""
echo "Moving windows to $MONITOR..."
for ID in "${WINDOW_IDS[@]}"; do
    niri msg action focus-window --id $ID
    niri msg action move-window-to-monitor $MONITOR 2>/dev/null || true
    sleep 0.1
done

# Helper: consume N windows into current column
consume_into_column() {
    local count=$1
    for i in $(seq 1 $count); do
        niri msg action consume-window-into-column
        sleep 0.15
    done
}

if [ "$MODE" = "tiled" ]; then
    echo ""
    echo "Applying tiled layout: $LAYOUT (${INSTANCE_COUNT}p)"

    # Ensure all windows are in tiling mode first
    for ID in "${WINDOW_IDS[@]}"; do
        niri msg action focus-window --id $ID
        niri msg action move-window-to-tiling 2>/dev/null || true
        sleep 0.05
    done
    sleep 0.3

    # Re-fetch window IDs in their column order (leftmost first)
    WINDOW_IDS=($(niri msg --json windows | jq -r '[.[] | select(.app_id == "gamescope")] | sort_by(.layout.pos_in_scrolling_layout[0]) | .[].id'))

    echo "Windows in column order: ${WINDOW_IDS[*]}"

    case "$LAYOUT" in
        fullscreen)
            echo "→ Single window at 100%"
            niri msg action focus-window --id ${WINDOW_IDS[0]}
            niri msg action set-column-width 100%
            ;;

        vertical|side_by_side)
            # N columns, each with 1 window, equal width
            WIDTH_PCT=$((100 / INSTANCE_COUNT))
            echo "→ ${INSTANCE_COUNT} columns (${WIDTH_PCT}% each)"
            for i in $(seq 0 $((INSTANCE_COUNT - 1))); do
                niri msg action focus-window --id ${WINDOW_IDS[$i]}
                niri msg action set-column-width ${WIDTH_PCT}%
            done
            ;;

        horizontal|stacked)
            # 1 column, N windows stacked (equal height auto-managed by niri)
            echo "→ 1 column with ${INSTANCE_COUNT} stacked (equal height)"
            niri msg action focus-window --id ${WINDOW_IDS[0]}
            consume_into_column $((INSTANCE_COUNT - 1))
            niri msg action set-column-width 100%
            ;;

        grid)
            if [ $INSTANCE_COUNT -ne 4 ]; then
                echo "Error: Grid layout only supports 4 players"
                exit 1
            fi
            # 2 columns, 2 windows each (2x2 grid)
            echo "→ 2x2 grid (2 columns, 2 stacked each)"
            # Stack P1+P2 in column 1
            niri msg action focus-window --id ${WINDOW_IDS[0]}
            sleep 0.1
            consume_into_column 1
            # Stack P3+P4 in column 2
            niri msg action focus-window --id ${WINDOW_IDS[2]}
            sleep 0.1
            consume_into_column 1
            # Set equal widths
            niri msg action focus-window --id ${WINDOW_IDS[0]}
            niri msg action set-column-width 50%
            niri msg action focus-window --id ${WINDOW_IDS[2]}
            niri msg action set-column-width 50%
            ;;

        *)
            echo "Error: Unknown layout '$LAYOUT'"
            echo "Available: fullscreen, vertical, horizontal, grid"
            exit 1
            ;;
    esac

    sleep 0.2
    echo ""
    echo "=== Final Tiled Layout ==="
    niri msg --json windows | jq '.[] | select(.app_id == "gamescope") | {id, col_row: .layout.pos_in_scrolling_layout, size: .layout.window_size}'

else
    # ================== FLOATING MODE ==================
    position_window() {
        local ID=$1
        local X=$2
        local Y=$3
        local W=$4
        local H=$5

        echo "  Window $ID -> ${W}x${H}+${X}+${Y}"

        niri msg action focus-window --id $ID
        niri msg action move-window-to-floating 2>/dev/null || true
        sleep 0.05

        niri msg action set-window-width $W
        niri msg action set-window-height $H
        sleep 0.05

        local CURRENT=$(niri msg --json windows | jq -r ".[] | select(.id == $ID) | .layout.tile_pos_in_workspace_view | \"\(.[0]) \(.[1])\"")
        local CX=$(echo $CURRENT | cut -d' ' -f1 | cut -d. -f1)
        local CY=$(echo $CURRENT | cut -d' ' -f2 | cut -d. -f1)
        local DX=$((X - CX))
        local DY=$((Y - CY))

        local X_ARG=$([ $DX -ge 0 ] && echo "+$DX" || echo "$DX")
        local Y_ARG=$([ $DY -ge 0 ] && echo "+$DY" || echo "$DY")
        niri msg action move-floating-window --id $ID -x $X_ARG -y $Y_ARG
    }

    echo ""
    echo "Applying floating layout: $LAYOUT (${INSTANCE_COUNT}p)"

    case "$LAYOUT" in
        fullscreen)
            position_window ${WINDOW_IDS[0]} $MON_X $MON_Y $MON_W $MON_H
            ;;

        vertical|side_by_side)
            # N columns, equal width
            CELL_W=$((MON_W / INSTANCE_COUNT))
            echo "→ ${INSTANCE_COUNT} columns (${CELL_W}px each)"
            for i in $(seq 0 $((INSTANCE_COUNT - 1))); do
                X=$((MON_X + i * CELL_W))
                position_window ${WINDOW_IDS[$i]} $X $MON_Y $CELL_W $MON_H
            done
            ;;

        horizontal|stacked)
            # N rows, equal height
            CELL_H=$((MON_H / INSTANCE_COUNT))
            echo "→ ${INSTANCE_COUNT} rows (${CELL_H}px each)"
            for i in $(seq 0 $((INSTANCE_COUNT - 1))); do
                Y=$((MON_Y + i * CELL_H))
                position_window ${WINDOW_IDS[$i]} $MON_X $Y $MON_W $CELL_H
            done
            ;;

        grid)
            if [ $INSTANCE_COUNT -ne 4 ]; then
                echo "Error: Grid layout only supports 4 players"
                exit 1
            fi
            HALF_W=$((MON_W / 2))
            HALF_H=$((MON_H / 2))
            echo "→ 2x2 grid"
            position_window ${WINDOW_IDS[0]} $MON_X $MON_Y $HALF_W $HALF_H
            position_window ${WINDOW_IDS[1]} $((MON_X + HALF_W)) $MON_Y $HALF_W $HALF_H
            position_window ${WINDOW_IDS[2]} $MON_X $((MON_Y + HALF_H)) $HALF_W $HALF_H
            position_window ${WINDOW_IDS[3]} $((MON_X + HALF_W)) $((MON_Y + HALF_H)) $HALF_W $HALF_H
            ;;

        *)
            echo "Error: Unknown layout '$LAYOUT'"
            exit 1
            ;;
    esac

    echo ""
    echo "=== Final Floating Layout ==="
    niri msg --json windows | jq '.[] | select(.app_id == "gamescope") | {id, pos: .layout.tile_pos_in_workspace_view, size: .layout.window_size}'
fi

echo ""
echo "Press Enter to clean up test windows..."
read
pkill -f "gamescope.*glxgears" 2>/dev/null || true
echo "Done."
