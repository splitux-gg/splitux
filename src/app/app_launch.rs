// Game setup and launch functions

use super::app::{InstanceFocus, MenuPage, Splitux};
use crate::config::save_cfg;
use crate::audio::AUDIO_MUTED_SENTINEL;
use crate::input::*;
use crate::launch::*;
use crate::monitor::get_monitors_sdl;
use crate::profiles::*;
use crate::util::*;

impl Splitux {
    pub fn start_game_setup(&mut self) {
        self.start_game_setup_injecting(None);
    }

    /// Start instance setup for the selected game. When `inject_path` is the evdev
    /// path of the device that activated the game (A on a controller, or a
    /// right-click with the mouse), that device is added as player 1 immediately —
    /// with its kb/mouse partner via `join_kbm_partners` — so activating a game
    /// drops you into a ready-to-launch seat instead of an empty setup screen. The
    /// path (not an index) is captured because `set_input_devices` rescans and
    /// re-sorts, which would invalidate a raw index.
    pub fn start_game_setup_injecting(&mut self, inject_path: Option<String>) {
        let h = &self.handlers[self.selected_handler];
        if h.steam_appid.is_none() && h.path_gameroot.is_empty() {
            msg(
                "Game root path not found",
                "Please specify the game's root folder by editing the handler.",
            );
            self.handler_edit = Some(h.clone());
            self.show_edit_modal = true;
            return;
        }
        self.instances.clear();
        // This launch starts as a single-game session with the picked game as
        // game 0. `add_game_to_session` appends more for multi-game.
        self.selected_games = vec![self.selected_handler];
        let devices = scan_input_devices(&self.options.pad_filter_type, &self.options.input_blacklist);
        self.set_input_devices(devices);
        self.monitors = get_monitors_sdl();
        self.profiles = scan_profiles(true);
        self.instance_add_dev = None;
        self.launch_option_index = 0;
        self.cur_page = MenuPage::Instances;

        // Auto-inject the activating device as the first player (resolved by path
        // against the freshly-rescanned list).
        let injected = inject_path
            .and_then(|p| self.input_devices.iter().position(|d| d.path() == p));
        if let Some(d) = injected {
            self.instances.push(crate::instance::Instance {
                devices: vec![d],
                game: 0,
                profname: String::new(),
                profselection: 0,
                monitor: 0,
                width: 0,
                height: 0,
                together: false,
                together_input: crate::instance::TogetherInput::Gamepad,
                together_seats: 0,
                local_input: false,
            });
            // kb/mouse activate as one I/O unit — pull in the partner.
            self.join_kbm_partners(0, d);
            // A non-gamepad seat needs input holding for the launch to bind it.
            if self.input_devices[d].device_type() != crate::input::DeviceType::Gamepad {
                self.options.input_holding = true;
            }
        }
        // Focus the device strip either way: with a player already injected, the
        // next A-press on ANOTHER device should create player 2 (the device-strip
        // path) rather than join it onto player 1's card (the focused-card path).
        self.instance_focus = InstanceFocus::Devices;
    }

    /// Prune `selected_games` to only the games an instance actually uses and
    /// compact `Instance.game` to match (first-seen order). Keeps the launch
    /// honest after free-form picker edits. No-op for a clean single-game setup.
    fn normalize_session_games(&mut self) {
        if self.selected_games.is_empty() {
            return;
        }
        let mut remap: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        let mut new_games: Vec<usize> = Vec::new();
        for inst in &self.instances {
            if let std::collections::hash_map::Entry::Vacant(e) = remap.entry(inst.game)
                && let Some(&h) = self.selected_games.get(inst.game) {
                    e.insert(new_games.len());
                    new_games.push(h);
                }
        }
        // No instances yet → leave selected_games as the picked set.
        if new_games.is_empty() {
            return;
        }
        for inst in &mut self.instances {
            inst.game = remap.get(&inst.game).copied().unwrap_or(0);
        }
        self.selected_games = new_games;
    }

    /// Handlers for this launch, in `Instance.game` order. Falls back to the
    /// single selected handler if `selected_games` wasn't initialized (defensive).
    fn session_handlers(&self) -> Vec<crate::handler::Handler> {
        if self.selected_games.is_empty() {
            return vec![self.cur_handler().to_owned()];
        }
        self.selected_games
            .iter()
            .map(|&i| self.handlers[i].clone())
            .collect()
    }

    pub fn prepare_game_launch(&mut self) {
        // Instance setup is scoped to the ONE game selected on the left panel
        // (the per-seat game picker was removed). Pin every seat to it at launch
        // so the scoped game always tracks the current left-panel selection, even
        // if it changed after setup started. `handler_lite` is its own one-off.
        if self.handler_lite.is_none() {
            self.selected_games = vec![self.selected_handler];
            for inst in &mut self.instances {
                inst.game = 0;
            }
        }

        // Prune any game no instance ended up using, and compact `Instance.game`
        // indices, so the launch never carries an empty game (which would reserve
        // a monitor / handler for zero windows).
        self.normalize_session_games();

        // Sizing, profile naming AND the per-game local-split collapse now happen
        // inside the shared launch-core (`run_launch`) — the SAME path the CLI
        // takes — instead of the GUI doing its own subset here. This is what makes
        // the bootstrap identical across presentation layers (and gives the GUI
        // local-split collapse, which it previously skipped).
        // The launch's handler set. A one-off `handler_lite` launch stays
        // single-game; otherwise use the session's `selected_games` (one entry for
        // a normal single-game setup, several for multi-game). `Instance.game`
        // indexes into this list.
        let handlers: Vec<crate::handler::Handler> = if let Some(h) = self.handler_lite.clone() {
            vec![h]
        } else {
            self.session_handlers()
        };

        let instances = self.instances.clone();
        let monitors = self.monitors.clone();
        let profiles = self.profiles.clone();
        // Parity with the CLI (which forces this on when --display is used): use
        // multi-monitor sizing whenever the seats span more than one distinct
        // display, not only when the SDL backend is on. The per-card Display
        // picker assigns `instance.monitor`, so without this the GUI sized every
        // seat to the primary monitor even when you'd spread them across outputs.
        let first_mon = self.instances.first().map(|i| i.monitor);
        let spans_monitors = self.instances.iter().any(|i| Some(i.monitor) != first_mon);
        let use_multimonitor = self.options.gamescope_sdl_backend || spans_monitors;
        let dev_infos: Vec<DeviceInfo> = self.input_devices.iter().map(|p| p.info()).collect();

        // Resolve audio assignments: session overrides take precedence over profile preferences
        let mut cfg = self.options.clone();
        for i in 0..self.instances.len() {
            // Check session override first
            if let Some(override_opt) = self.audio_session_overrides.get(&i) {
                match override_opt {
                    Some(sink_name) => {
                        cfg.audio.default_assignments.insert(i, sink_name.clone());
                        println!(
                            "[splitux] Applied session audio override for instance {}: {}",
                            i, sink_name
                        );
                    }
                    None => {
                        // Explicit mute - use sentinel value so audio routes to null sink
                        cfg.audio
                            .default_assignments
                            .insert(i, AUDIO_MUTED_SENTINEL.to_string());
                        println!(
                            "[splitux] Instance {} audio muted (session override)",
                            i
                        );
                    }
                }
                continue;
            }

            // Fall back to profile preference
            if let Some(sink_name) = self.profile_audio_prefs.get(&i) {
                cfg.audio.default_assignments.insert(i, sink_name.clone());
                println!(
                    "[splitux] Applied profile audio preference for instance {}: {}",
                    i, sink_name
                );
            }
        }
        let _ = save_cfg(&cfg);

        // Capture master profile for use in launch thread
        let master_profile = cfg.master_profile.clone();

        // Fresh "windows up" signal for this launch; the UI clears the overlay
        // and detaches once the launch thread sets it.
        self.launch_ready = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let launch_ready = self.launch_ready.clone();

        self.cur_page = MenuPage::Games;
        self.spawn_task(
            "Launching...\n\nDon't press any buttons or move any analog sticks or mice.",
            move || {
                // Shared launch-core facade (also used by the headless CLI):
                // collapse-per-game → size → name → run_session. User-facing
                // failures pop a modal here; the CLI routes them to stderr.
                run_launch(
                    &handlers,
                    instances,
                    monitors,
                    &profiles,
                    &dev_infos,
                    &cfg,
                    use_multimonitor,
                    master_profile.as_deref(),
                    &launch_ready,
                    &|title, body| msg(title, body),
                );
            },
        );
    }
}
