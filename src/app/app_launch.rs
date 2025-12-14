// Game setup and launch functions

use std::thread::sleep;

use super::app::{InstanceFocus, MenuPage, PartyApp};
use super::config::save_cfg;
use crate::input::*;
use crate::instance::*;
use crate::launch::*;
use crate::monitor::get_monitors_sdl;
use crate::profiles::*;
use crate::save_sync;
use crate::util::*;

macro_rules! cur_handler {
    ($self:expr) => {
        &$self.handlers[$self.selected_handler]
    };
}

impl PartyApp {
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
            cur_handler!(self).to_owned()
        };

        let instances = self.instances.clone();
        let monitors = self.monitors.clone();
        let dev_infos: Vec<DeviceInfo> = self.input_devices.iter().map(|p| p.info()).collect();

        let cfg = self.options.clone();
        let _ = save_cfg(&cfg);

        self.cur_page = MenuPage::Games;
        self.spawn_task(
            "Launching...\n\nDon't press any buttons or move any analog sticks or mice.",
            move || {
                sleep(std::time::Duration::from_secs_f32(1.5));

                if let Err(err) = setup_profiles(&handler, &instances) {
                    println!("[splitux] Error setting up profiles: {}", err);
                    msg("Failed setting up profiles", &format!("{err}"));
                    return;
                }

                // Copy original saves to all profiles before launch
                if !handler.original_save_path.is_empty() {
                    if let Err(err) = save_sync::copy_original_saves_to_all_profiles(&handler, &instances) {
                        println!("[splitux] Warning: Failed to copy original saves: {}", err);
                        // Continue anyway - this is non-fatal
                    }
                }

                // Note: fuse_overlayfs_mount_gamedirs is now called inside launch_cmds
                // with proper Goldberg overlay support
                if let Err(err) = launch_game(&handler, &dev_infos, &instances, &monitors, &cfg) {
                    println!("[splitux] Error launching instances: {}", err);
                    msg("Launch Error", &format!("{err}"));
                }

                // Sync saves back from the first named profile after game exits
                if handler.save_sync_back {
                    if let Err(err) = save_sync::sync_saves_back(&handler, &instances) {
                        println!("[splitux] Error syncing saves back: {}", err);
                        msg("Save Sync Error", &format!("Failed to sync saves back: {err}"));
                    }
                }

                // WM teardown is now handled inside launch_game
                if let Err(err) = remove_guest_profiles() {
                    println!("[splitux] Error removing guest profiles: {}", err);
                    msg("Failed removing guest profiles", &format!("{err}"));
                }
                if let Err(err) = clear_tmp() {
                    println!("[splitux] Error removing tmp directory: {}", err);
                    msg("Failed removing tmp directory", &format!("{err}"));
                }
            },
        );
    }
}
