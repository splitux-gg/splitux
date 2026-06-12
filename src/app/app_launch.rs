// Game setup and launch functions

use super::app::{InstanceFocus, MenuPage, Splitux};
use crate::config::save_cfg;
use crate::audio::AUDIO_MUTED_SENTINEL;
use crate::input::*;
use crate::instance::*;
use crate::launch::*;
use crate::monitor::get_monitors_sdl;
use crate::profiles::*;
use crate::util::*;

impl Splitux {
    pub fn start_game_setup(&mut self) {
        let h = &self.handlers[self.selected_handler];
        if h.steam_appid.is_none() && h.path_gameroot.is_empty() {
            msg(
                "Game root path not found",
                "Please specify the game's root folder by editing the handler.",
            );
            self.handler_edit = Some(h.clone());
            self.show_edit_modal = true;
        } else {
            self.instances.clear();
            self.input_devices = scan_input_devices(&self.options.pad_filter_type);
            self.refresh_device_display_names();
            self.monitors = get_monitors_sdl();
            self.profiles = scan_profiles(true);
            self.instance_add_dev = None;
            self.instance_focus = InstanceFocus::Devices;
            self.launch_option_index = 0;
            self.cur_page = MenuPage::Instances;
        }
    }

    pub fn prepare_game_launch(&mut self) {
        if self.options.gamescope_sdl_backend {
            set_instance_resolutions_multimonitor(
                &mut self.instances,
                &self.monitors,
                &self.options,
            );
        } else {
            set_instance_resolutions(&mut self.instances, &self.monitors[0], &self.options);
        }
        set_instance_names(&mut self.instances, &self.profiles);

        let handler = if let Some(h) = self.handler_lite.clone() {
            h
        } else {
            self.cur_handler().to_owned()
        };

        let instances = self.instances.clone();
        let monitors = self.monitors.clone();
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
                // Shared launch core (also used by the headless CLI). User-facing
                // failures pop a modal here; the CLI routes them to stderr.
                run_session(
                    &handler,
                    &instances,
                    &monitors,
                    &dev_infos,
                    &cfg,
                    master_profile.as_deref(),
                    &launch_ready,
                    &|title, body| msg(title, body),
                );
            },
        );
    }
}
