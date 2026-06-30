# splitux multi-game mode — implementation plan

Goal: one `splitux launch` invocation that runs several **different** games concurrently,
each with its own seats/players, coordinated by one session owner — replacing the current
"run N independent `splitux` processes" approach, which race-fails (two independent processes
each set up niri/waybar/monitors and bring up gamescope; one gamescope hangs in early init).

> ## STATUS (current — multi-game is SHIPPED, CLI-only)
> The engine described below is **implemented**: `Instance.game` units, per-game
> backends/overlays/ports, per-unit sub-slices, `run_launch(&[Handler])`, per-monitor
> tiling, and the full CLI grammar (`--game` repeatable, `game=`/`<game>=` tags). The one
> design change from the original plan: **multi-game is exposed only through the CLI.** The
> GUI and TUI are deliberately **single-game** (explicit, interactive setup); a per-card
> Game picker was briefly added to the GUI and then removed. The front-ends feed `run_launch`
> a single handler; the CLI is the surface that carries multi-game (for scripting/automation).
> See `docs/multi-game-smoke-checklist.md` and memory `splitux-instance-setup-simplification`.
> Step 8 below ("GUI: …Game tag") is therefore **intentionally not done** for the GUI/TUI —
> only the CLI grammar landed.

## Ground truth (today)

One `splitux launch` = one process = one `Handler` threaded as `&h`:
`cli::launch` (one handler) → `run_session(&handler, &instances, …)` (`session.rs:30`) →
`launch_game(h, …)` (`execute.rs:89`) → `launch_cmds(h, …)` (`build_cmds.rs:34`) → per-instance
`Command`s spawned **serially** (`execute.rs:291`) with `vulkan_init_delay=6s` after each.

Crucially the single process **already runs N concurrent gamescopes** (one per `Instance`) under
one niri setup, one waybar hide/restore, one slice, one `launch_ns`, one `setup_together_seats`
with globally-unique seat indices. That's the split-screen path and it works. The only thing that
is "one game" is the `Handler`; `Instance` (`instance.rs:43`) has no game reference.

## Foundational layer: ONE shared launch-core, three thin presentations (CLI/TUI/GUI)

CLI, TUI, and GUI currently each build the launch their own way → they diverge and cause side-effects.
Abstract the launch logic **one ordinal level above** all three: a single **launch-core** that takes a
minimal contract and "dumbly" launches without heavy config layering. CLI/TUI/GUI become **presentation
layers** that only collect input and call the core — same base, no per-surface variance. The CLI must
expose **every feature the GUI/infra already has** (leave nothing out) but with **easy default passes**
(a bare `game + profile + io` just works).

**Minimal launch contract (per player/unit):**
- `game` — required.
- `profile` — required.
- `io` — required **only if local** (local is io-sensitive). For **together it does NOT matter**:
  everything is passed in via the seat, so the binding can be defaulted/ignored.

**Together vs local is a single logic gate in the core:**
- **Together:** display / positioning / layout / splitting are **irrelevant** (surface is streamed, not
  locally consumed) → skip them entirely. Bare minimum = game + profile.
- **Local:** `io` required; display/layout/splitting **only matter here**. If **no display is given →
  default to FULLSCREEN on the first target display.** Splitting/positioning handled in this branch only.

So multi-game orchestration sits in the core: it builds the unit list, resolves each unit's topology from
its handler, gates together-vs-local config per the above, then runs the single-process spawn loop.

## Mental model: a game is a UNIT (backend abstracted from the render pipeline)

Each game is a self-contained **unit** whose handler declares its coop topology:
- **local coop** (`coop_mode: local-split`): **N seats → 1 instance** (one render surface, N controllers
  folded into the one game process via `collapse_for_local_split`).
- **online coop**: **N instances → 1 seat each** (N render surfaces, networked via the backend).

The seat↔instance mapping is resolved **inside the unit** (from the handler) before anything renders,
so the **display/rendering pipeline is backend-agnostic** — it just consumes "M render surfaces, each
with its input." Backend (goldberg / EOS / local) is orthogonal and per-unit.

Multi-game = **a list of units**. Lifecycle is **per-unit process-tree tracing**: each unit owns its
parent→child subtree (game proc(s) + gamescope(s) + seat-streamers), so supervision and teardown are
independent per unit. This maps directly onto splitux's existing slice/scope model — give each unit its
own **sub-slice** under the launch slice; teardown = "stop that unit's slice" (PID tracing for free via
the cgroup), and one unit exiting/crashing doesn't disturb the others.

### ✅ SHIPPED (2026-06-27): the unit sub-slice spine — this is the layer the abstraction lives in
The per-unit sub-slice is the foundational boundary the rest hangs off (per the user: "Goldberg and other
backends scope to the respective parent PID and the children — that's the layer the abstraction works in").
Implemented in `src/launch/operations/scope.rs`:
- **`unit_slice_name(launch_id, game)` → `splitux-<id>-g<g>.slice`.** systemd derives slice hierarchy from
  `-`, so it nests automatically: `splitux.slice ▸ splitux-<id>.slice (launch) ▸ splitux-<id>-g<g>.slice
  (unit)`. The unit's instance + seat scopes JOIN this slice, so the whole game is one cgroup subtree.
- **`wrap_command`/`wrap_seat_command` gained a `game` param** → unit names `…-g<g>-i<k>.scope` /
  `…-g<g>-seat<k>.scope`, `--slice=…-g<g>.slice`. Instance/seat idx stay GLOBAL (no cross-unit name
  collision). Callers pass `instances[i].game` (`execute.rs`) and `instance.game` (`together.rs`).
- **`stop_unit_slice(launch_id, game)`** = independent per-unit teardown (stop one game, leave the rest);
  whole-launch `stop_slice` still cascades to every unit. (Wired into the supervise loop in a later pass.)
- **Verified at the systemd level** (live smoke): scope lands at cgroup `/splitux/<id>/g0`, parent chain
  scope→unit→launch correct, `stop <launch>.slice` cascades to unit+scope+process. **Single-game is one
  unit `g0`** — functionally byte-identical (teardown cascade unchanged; the niri window filter matches
  `cgroup.contains("splitux-<ns>")`, still a substring after `-g0` insertion; `owner_pid`/sweep key off
  the `<pid>_` prefix, robust to `-g<g>`). All unit tests pass + new `unit_slice_nests_under_launch_slice`.

### Goldberg / backend isolation between concurrent units — DECIDED (2026-06-27)
Confirmed in code: goldberg LAN discovery is **targeted unicast** — it writes `custom_broadcasts.txt` as an
explicit `127.0.0.1:<port>` list (`backend/goldberg/operations/write_settings.rs:64`) and unicasts to
exactly those, NOT a subnet broadcast. Therefore:
- **DEFAULT = per-unit port grouping** (light, EOS-safe): build each instance's `broadcast_ports` from
  **same-game peers only** (`goldberg.rs:176` currently "all OTHER instances" → "all other instances IN
  THE SAME GAME"). On shared `127.0.0.1`, game-0 never contacts game-1's ports → lobbies stay disjoint
  with no kernel isolation. `instance_ports` must be GLOBAL (`BASE_PORT + global_i`) so units don't reuse
  ports. (This is part of step 4's per-game-group overlay creation.)
- **OPT-IN = per-unit netns** only when a handler sets `goldberg.bridged_lan`: that unit's instances get
  their own netns/bridge (distinct LAN IPs). Make `bridged` per-unit (today it's all-or-nothing across the
  whole launch — `execute.rs:269`). EOS-localhost games can't use netns (existing warning) → they rely on
  port grouping.

Implementation-wise this *is* approach A: `Instance.game` index **= unit membership**. The union of all
units' instances feeds the one proven split-screen spawn loop (single process, serialized bring-up =
race fixed), while batch ops / saves / **teardown** group by unit (each unit's own local instance
indices, namespaced for global uniqueness of dirs/seats/nodes).

## Recommended approach: (A) per-instance handler, in ONE owning process

Add a game reference to `Instance`, thread `&[Handler]` (indexed per instance) through
build_cmds/execute/backends, keep the single `run_session → launch_game → one-spawn-loop`.

**Why A, not a sub-session orchestrator (B):** the concurrency fix falls out for free. Today's
race is N *separate processes* each doing niri/waybar/gamescope bring-up concurrently. A collapses
N games into the one process that already serializes gamescope spawns (the 6s vulkan delay) under
one WM/waybar/slice — the proven split-screen path. **Multi-game mode = split-screen with a
per-instance handler.** B would duplicate the racy bring-up and still need the ~83-site refactor.

## CLI grammar (keep single-game form byte-for-byte)

```
# existing — unchanged
splitux launch --game Satisfactory --player profile=Gabe,input=local:kbm --player profile=Ruth,input=local:gamepad

# new multi-game
splitux launch --game Satisfactory --game Palworld \
  --player game=Satisfactory,profile=Gabe,input=local:gamepad \
  --player game=Satisfactory,profile=Ruth,input=local:gamepad \
  --player game=Palworld,profile=Alice,input=together:gamepad \
  --layout Satisfactory=vertical --display Satisfactory=DP-2 --display Palworld=HDMI-A-1
```
- `game: String` → `Vec<String>` (`cli.rs:82`). 1 ⇒ single-game; ≥2 ⇒ multi-game.
- `--player` gains optional `game=<name>` (required in multi-game, omitted binds to the sole game).
- `--layout`/`--display` become game-tagged `<GAME>=<value>` in multi-game; bare forms still work for one game.

## Files / functions to change

- **instance.rs**: add `pub game: usize` (default 0); fix the 2 constructors + test builder.
- **session.rs `run_session`**: `handler: &Handler` → `handlers: &[Handler]`; profiles + save-init + save-back loop **per game-group** (master becomes per-game). Rest is session-level, unchanged.
- **execute.rs `launch_game`**: `h` → `handlers`; helper `h_of(i)=&handlers[instances[i].game]`.
  - Per-game/instance fixes: gptokeyb (per group), `setup_together_seats` seat **label** per game,
    `bridged_lan`/netns **per instance** (mixed bridged/non-bridged supported), `redirect_stdout`,
    `disable_bwrap`/`input_isolation` via `h_of(i)`.
  - Session-level (unchanged): audio, scope/slice/launch_ns, WM detect, spawn loop + delays,
    supervise loop, teardown.
- **build_cmds.rs `launch_cmds`**: `h` → `handlers`; inside the loop `let h = &handlers[instance.game];`.
  Move `validate_runtime`/win/exec/runtime into the loop. Batch ops (`create_backend_overlays`,
  `fuse_overlayfs_mount_gamedirs`, photon configs) run **per game-group keyed by GLOBAL instance
  index** (so `game-{i}` dirs stay disjoint). `$INSTANCECOUNT`/`$INSTANCENUM` become **per-game**.
  Goldberg "first instance gets real steam-id" keys off **per-game** first index, not global `i==0`.
- **backend.rs / overlays.rs**: overlay creation per game-group with global indices.
  - **Step-4 design (mapped 2026-06-27):** the grouping is not only about disjoint scratch dirs — it is
    **semantically required for goldberg lobby isolation**. `GoldbergBackend::create_all_overlays` builds
    `broadcast_ports = every OTHER instance's port` (`backend/goldberg.rs:176`) so peers discover each
    other on the LAN/P2P bus. In multi-game that "every other" must be **scoped to the same game** — a
    game-1 instance must NOT broadcast into game-2's lobby. So `create_backend_overlays` must iterate
    **per game-group** (handler = `handlers[g]`, instances = that game's subset) and merge results into a
    global-indexed `Vec<Vec<PathBuf>>`.
  - **Global-index threading:** every per-instance overlay dir names by index and must use the GLOBAL
    index to stay collision-free across games: `{backend}-overlay-{i}` (`backend/operations/overlay.rs:16`),
    `standalone-{i}` (`backend/standalone.rs:92`), photon (`backend/photon/pipelines/setup.rs:105,137`),
    facepunch (`backend/facepunch/pipelines/setup.rs:38`), and the mount/work dirs `game-{i}`/`work-{i}`
    (`launch/operations/overlays.rs:73` — must match `build_cmds.rs:92`'s read). The clean thread is to
    give each per-game call its group's GLOBAL indices (e.g. a parallel `&[usize]` or a `global_idx` field
    on `GoldbergConfig`/equivalents) replacing the bare `enumerate()` `i`. Single-game: indices are
    `0..n`, byte-identical. Touches the backend trait + 5 impls (goldberg/photon/facepunch/standalone/eos).
  - **Done in step 3 already:** per-game `$INSTANCECOUNT/NUM` + goldberg in-loop first-index
    (`build_cmds.rs:242`) via the pure `per_game_instance_numbering` helper. The MATCHING `goldberg.rs:190`
    config first-index (`i == 0`) still keys off the global first instance — step 4 regroups it per game,
    keeping the two in agreement (they already agree for single-game).
- **wm.rs / niri.rs / presets.rs**: layout becomes **per-monitor (≈ per-game)**. `LayoutContext`
  gains a per-monitor preset; niri split path groups windows by `inst.monitor` and tiles per monitor.
  Fullscreen-per-monitor path already works → ship that first.
- **instance.rs `set_instance_resolutions_multimonitor`**: consult a **per-monitor** preset (already
  sizes by per-monitor count).
- **proton.rs `get_prefix_path`**: namespace prefix by `(profname, game)` **only in multi-game** (avoid
  invalidating existing single-game prefixes); else same-profile-across-games share + fight a prefix.

## Classification of the ~83 handler sites
- **Per-game** (→ `handlers[inst.game]`): appid, exec/cwd/working_dir, runtime+validate, args+`$`subst,
  env, sdl2_override, win(), all goldberg.* (envs/save_path/GseAppPath/GseSavePath/steamclient/
  p2p_bridge/bridged_lan), eos identity, facepunch/photon/bepinex, game_null_paths, disable_bwrap,
  input_isolation, has_gptokeyb, save-anchor fields, seat invite label (`h.name`).
- **Session-level** (one call): audio, scope/slice/launch_ns, WM detect, spawn loop+delays, supervise,
  teardown, guest-profile removal, scratch unmount, marker.

## Why the race goes away (1 process vs N)
| racy resource | N processes (today) | under A (1 process) |
|---|---|---|
| niri setup | each process fights outputs/windows | `wm.setup` once; windows filtered by this launch's scope cgroup |
| waybar hide/restore | two `pkill`+restore race the shared state file | one hide/restore, single owner of state |
| gamescope early-init | two gamescopes contend → one hangs before Vulkan | spawns serialized in one loop (6s vulkan delay) — split-screen path |
| pipewire nodes | cross-process node ordering can clash | globally-unique instance indices by construction |
| scope/launch_ns/tmp | N main-scope sweeps + re-execs | one main scope, one re-exec, `game-{global_i}` unique |

## Risks
- Proton prefix sharing for reused named profiles → namespace by game (above).
- Heterogeneous backends (one bridged_lan, one EOS-localhost) → `bridged` per-instance; warn per game.
- niri per-monitor tiling is the trickiest WM change → ship fullscreen-per-monitor first.
- `$INSTANCECOUNT/NUM` semantics change to per-game → verify no handler relies on global count.
- More games than monitors → degraded/round-robin layout → gate or warn.

## Incremental, testable ordering (each step keeps single-game identical)
1. `Instance.game: usize` (default 0) + fix constructors. No behavior change.
2. Thread `&[Handler]` through run_session/launch_game/launch_cmds; callers pass a length-1 slice; internally `handlers[0]`. No behavior change.
3. In-loop `h.*` → `handlers[inst.game].*`; per-game `$INSTANCECOUNT/NUM` + goldberg first-index. Single-game byte-identical (add a regression test).
4. Batch ops (overlays/mount/photon/profiles/save/gptokeyb) per game-group with global indices.
5. `bridged`/`redirect_stdout`/isolation per-instance; per-game seat labels.
6. Per-monitor `LayoutContext` + resolutions; niri fullscreen-per-monitor first, then per-monitor tiling.
7. Proton prefix game-namespacing in multi-game.
8. CLI: `game: Vec<String>`, `game=` player tag, game-tagged `--layout`/`--display`, per-game master/save-anchor; per-game `collapse_for_local_split`.
9. E2E: two games fullscreen one-per-monitor (proves the concurrency fix) → split layouts → together+local mix.

## Critical files
- src/launch/pipelines/build_cmds.rs · execute.rs · session.rs
- src/cli.rs · src/instance.rs
- (secondary) src/wm.rs, src/wm/niri.rs, src/wm/presets.rs, src/backend.rs,
  src/launch/operations/overlays.rs, src/together.rs, src/proton.rs
