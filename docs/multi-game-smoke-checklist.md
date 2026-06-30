# Multi-game + launch-core unification — what changed & what to smoke

Branch `feat/working-dir-handler-field`, uncommitted. Build clean, **149 unit tests pass**.
Single-game is designed to be **byte-identical** throughout — the #1 smoke priority is proving that.

> NOTE: some changed files also carry PRE-EXISTING branch WIP that is NOT part of this work
> (steamless DRM strip `goldberg/operations/steamless.rs`, `util.rs`, `README.md`, the
> proton-by-profname / per-instance-sink bits). The untracked `docs/input-device-troubleshooting.md`
> + `docs/udev/` are from the earlier OC2 session. Everything below is the multi-game/unification work.

---

## A. WHAT WAS ADJUSTED

### 1. Multi-game core — a game is a "unit" (`Instance.game`)
- `instance.rs` — added `pub game: usize` (unit membership). `device_dispatch.rs`, `cli.rs`, `together.rs` constructors set it.
- `build_cmds.rs` — per-instance `h = &handlers[instance.game]`; `win`/`exec`/`runtime`/`validate_runtime` moved into the loop; per-game `$INSTANCECOUNT/$INSTANCENUM` + goldberg in-loop first-index via new pure helper.
- `launch/pure/numbering.rs` (NEW) — `per_game_instance_numbering()` + 4 regression tests.
- `session.rs` / `execute.rs` — pipeline threads `&[Handler]` (was single `&Handler`).

### 2. Per-game backends + goldberg lobby isolation
- `backend.rs` — `create_backend_overlays(handlers, instances)` groups per game, GLOBAL-indexed overlay dirs, per-game `is_windows`.
- Backend trait + all 5 impls (`goldberg.rs`, `eos/pipelines.rs`, `facepunch/pipelines/setup.rs`, `photon/pipelines/setup.rs`, `standalone.rs`) — thread `global_indices` for collision-free dirs/ports.
- `goldberg.rs` / `goldberg/pipelines/setup.rs` — `broadcast_ports` = same-game peers only; `instance_ports` = GLOBAL (so two games never share a port). **This is the multi-game lobby isolation.**
- `overlays.rs` — `fuse_overlayfs_mount_gamedirs(handlers, …)` per-instance handler; skips non-saved games; per-game patch dirs.
- `gptokeyb/operations.rs` + `execute.rs` — gptokeyb per game, global-indexed virtual-device names.

### 3. Per-unit sub-slice spine (the "scope to parent PID + children" layer)
- `scope.rs` — `unit_slice_name()` → `splitux-<id>-g<g>.slice`; `wrap_command`/`wrap_seat_command` take a `game` param → instance/seat scopes nest under the unit slice; new `stop_unit_slice()`. Validated live at the systemd level (cgroup nesting + cascade teardown).
- `execute.rs` — `bridged_lan` per-unit (netns opt-in only when a handler sets it); per-instance `redirect_stdout`/isolation; per-game seat labels.
- `together.rs` — `setup_together_seats` takes per-game labels; `wrap_seat_command` gets the seat's game.

### 4. Proton prefix namespacing
- `proton.rs` — `get_prefix_path(cfg, profname, game)`: game 0 keeps the legacy path (single-game prefixes never re-init); games ≥1 get `-g<g>`.

### 5. CLI grammar (`cli.rs`)
- `--game` repeatable (`Vec<String>`); `--player game=<name>` tag; game-tagged `--display`/`--layout`; multi-game defaults each game to its own monitor round-robin.
- Single-game form unchanged.

### 6. Launch-core UNIFICATION
- `session.rs` — new `run_launch` facade: **collapse-per-game → size → name → run_session**. `run_session` no longer re-exported (`launch.rs`/`pipelines.rs`) — `run_launch` is the only public entry.
- `together.rs` — `collapse_instances_per_game` (per-game local-split fold).
- `cli.rs` + `app_launch.rs` — CLI and GUI both call `run_launch`. TUI shells out to the CLI. **The GUI was the divergent path (single-game, skipped collapse); now aligned.**

### 7. Per-monitor tiling (`wm/niri.rs`)
- Split path groups windows by monitor: single-monitor path extracted **verbatim** (`position_windows_single_monitor`, byte-identical); multi-monitor branch fullscreens a lone window per output, tiles shared ones (`tile_windows_on_monitor`).

### 8. GUI / TUI are single-game BY DESIGN (multi-game is CLI-only)
> **Reversed since this checklist was first written.** A per-instance "Game:" picker
> was briefly built into the GUI, then deliberately removed: instance setup is now
> **single-game-scoped** in both the GUI and the TUI (one game per session,
> configured explicitly), because the front-ends are meant to be explicit and
> interactive. **Multi-game is a CLI capability** — that's the surface used for
> scripted/automated launches. See memory `splitux-instance-setup-simplification`.
- The multi-game **engine** is fully live and is what carries the support: `Instance.game`,
  per-game backends/overlays/ports, per-unit sub-slices, `run_launch(&[Handler])`,
  per-monitor tiling, and the CLI grammar (§1–§7). Nothing about the engine was reverted.
- The GUI keeps `selected_games: Vec<usize>` + `prepare_game_launch`, but it always
  pins the session to the single left-panel game (`selected_games = [selected_handler]`,
  every `instance.game = 0`). The per-card Game dropdown and `InstanceCardFocus::Game`
  were removed. The TUI likewise builds a single-game launch (with layout + per-player
  display pickers).
- To smoke multi-game, use the **CLI** (`splitux launch --game A --game B …`), not the GUI/TUI.

---

## B. WHAT NEEDS TO BE SMOKED

### ✅ Already validated (no action needed)
- Release build clean, 149 unit tests pass.
- CLI: all `list` subcommands; all 6 multi-game arg parse/reject paths; single-game grammar.
- TUI: starts/renders; `launch_args` shell-out compatible with the new CLI.

### 🔶 NEEDS A LIVE SMOKE (priority order)

1. **Single-game regression (CRITICAL — prove nothing broke).**
   - CLI: `splitux launch --game <G> --player profile=<P>,input=local:gamepad` → boots, plays, tears down clean.
   - A goldberg local-split couch game (e.g. OC2) → collapse still folds to one instance.
   - A together seat → seat-streamer + invite still work.
   - Watch: one `splitux-<id>-g0.slice` now wraps it (was `-i0` directly) — teardown must still cascade.

2. **Multi-game, two games (THE new feature).** Needs 2 monitors for one-per-monitor.
   - `splitux launch --game A --game B --player game=A,… --player game=B,… --display A=DP-2 --display B=DP-3`
   - Watch: two gamescopes come up **without the old concurrent-launch race**; two unit slices `…-g0.slice` + `…-g1.slice`; `game-0`/`game-1` + `goldberg-overlay-0/1` dirs disjoint; if both goldberg LAN games, **lobbies stay separate** (same-game broadcast only).

3. **Per-unit teardown.** Kill/exit one game's slice → the other keeps running undisturbed.

4. **Per-monitor split tiling (step 6).** Two instances sharing one monitor in a multi-game launch → tiled per that monitor; lone-on-a-monitor → fullscreen. (Single-monitor split is verbatim-unchanged, lower risk.)

5. **GUI single-game (the facade rewire).**
   - GUI launches a normal single game exactly as before (it now routes through `run_launch`).
   - A local-split game launched from the GUI now **collapses** (it didn't before) — confirm couch co-op still works from the GUI.
   - The session always pins to the single left-panel game (`selected_games = [selected_handler]`); there is no per-card Game picker.

6. **GUI / TUI multi-game — N/A.** Multi-game is CLI-only by design (item #2 above). The GUI and TUI build single-game launches; there's nothing multi-game to smoke in them.

7. **Proton multi-game prefix.** Same profile name across two Windows games → game 1 gets `<prof>-g1` prefix (no prefix fight). Single-game prefix path unchanged (no re-init). (Multi-game launches come from the CLI.)

### ⛔ NOT built (don't smoke — not there yet)
- Multi-game save anchoring (single-game only; multi-game ignores `--save-anchor` with a note).
