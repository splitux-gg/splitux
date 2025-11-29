// Core app structure and main update loop

use std::collections::HashMap;

use super::config::*;
use super::focus::FocusManager;
use crate::handler::*;
use crate::input::*;
use crate::instance::*;
use crate::monitor::Monitor;
use crate::profiles::*;
use crate::util::*;

use eframe::egui;

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum FocusPane {
    GameList,   // Left panel - game selection
    ActionBar,  // Center panel - Play, Profile, Edit buttons
}

#[derive(Eq, PartialEq, Debug)]
pub enum MenuPage {
    Games,     // Combined home + profiles view
    Settings,
    Instances, // Controller assignment screen (enters when "Play" pressed)
}

pub struct PartyApp {
    pub installed_steamapps: Vec<Option<steamlocate::App>>,
    pub needs_update: bool,
    pub options: PartyConfig,
    pub cur_page: MenuPage,
    pub infotext: String,

    pub monitors: Vec<Monitor>,
    pub input_devices: Vec<InputDevice>,
    pub device_monitor: Option<DeviceMonitor>,
    pub instances: Vec<Instance>,
    pub instance_add_dev: Option<usize>,
    pub profiles: Vec<String>,
    pub game_profiles: HashMap<String, usize>, // Maps handler path -> selected profile index

    pub handlers: Vec<Handler>,
    pub selected_handler: usize,
    pub handler_edit: Option<Handler>,
    pub handler_lite: Option<Handler>,
    pub show_edit_modal: bool,

    // Focus management for spatial controller navigation
    pub focus_manager: FocusManager,
    pub activate_focused: bool, // Set to true when A button pressed

    // Pane-based focus for Games page (simpler than grid-based FocusManager)
    pub focus_pane: FocusPane,
    pub action_bar_index: usize, // 0=Play, 1=Profile, 2=Edit

    // Profile dropdown state (opened with Y button)
    pub profile_dropdown_open: bool,
    pub profile_dropdown_selection: usize, // Temporary selection while dropdown is open
    pub show_new_profile_dialog: bool,

    pub loading_msg: Option<String>,
    pub loading_since: Option<std::time::Instant>,
    #[allow(dead_code)]
    pub task: Option<std::thread::JoinHandle<()>>,
}

impl PartyApp {
    pub fn new(monitors: Vec<Monitor>, handler_lite: Option<Handler>) -> Self {
        let options = load_cfg();
        let input_devices = scan_input_devices(&options.pad_filter_type);
        let handlers = match handler_lite {
            Some(_) => Vec::new(),
            None => scan_handlers(),
        };
        let cur_page = match handler_lite {
            Some(_) => MenuPage::Instances,
            None => MenuPage::Games,
        };

        // Initialize device hotplug monitor
        let device_monitor = match DeviceMonitor::new() {
            Ok(m) => {
                println!("[splitux] udev: Device hotplug monitor initialized");
                Some(m)
            }
            Err(e) => {
                eprintln!("[splitux] udev: Failed to initialize device monitor: {}", e);
                None
            }
        };

        let profiles = scan_profiles(false);
        let mut app = Self {
            installed_steamapps: get_installed_steamapps(),
            needs_update: false,
            options,
            cur_page,
            infotext: String::new(),
            monitors,
            input_devices,
            device_monitor,
            instances: Vec::new(),
            instance_add_dev: None,
            profiles,
            game_profiles: HashMap::new(),
            handlers,
            selected_handler: 0,
            handler_edit: None,
            handler_lite,
            show_edit_modal: false,
            focus_manager: FocusManager::new(),
            activate_focused: false,
            focus_pane: FocusPane::GameList,
            action_bar_index: 0,
            profile_dropdown_open: false,
            profile_dropdown_selection: 0,
            show_new_profile_dialog: false,
            loading_msg: None,
            loading_since: None,
            task: None,
        };

        app.spawn_task("Checking for updates", move || {
            app.needs_update = check_for_splitux_update();
        });

        app
    }
}

impl eframe::App for PartyApp {
    fn raw_input_hook(&mut self, ctx: &egui::Context, raw_input: &mut egui::RawInput) {
        if !raw_input.focused || self.task.is_some() {
            return;
        }
        match self.cur_page {
            MenuPage::Instances => self.handle_devices_instance_menu(),
            _ => self.handle_gamepad_gui(ctx, raw_input),
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll for device hotplug events
        self.poll_device_events();

        // Reset focus state at start of frame
        self.focus_manager.begin_frame();

        // Enhance focus visuals for controller navigation
        ctx.style_mut(|style| {
            // Make focus stroke more visible (bright cyan outline)
            style.visuals.widgets.hovered.bg_stroke =
                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 255));
            style.visuals.selection.stroke =
                egui::Stroke::new(2.0, egui::Color32::from_rgb(100, 200, 255));
            // Improve keyboard focus visuals
            style.visuals.widgets.active.bg_stroke =
                egui::Stroke::new(3.0, egui::Color32::from_rgb(100, 200, 255));
        });

        // Enable keyboard focus navigation
        ctx.options_mut(|opt| {
            opt.input_options.line_scroll_speed = 40.0;
        });

        egui::TopBottomPanel::top("menu_nav_panel").show(ctx, |ui| {
            if self.task.is_some() {
                ui.disable();
            }
            self.display_panel_top(ui);
        });

        if !self.is_lite() {
            egui::SidePanel::left("games_panel")
                .resizable(false)
                .exact_width(200.0)
                .show(ctx, |ui| {
                    if self.task.is_some() {
                        ui.disable();
                    }
                    self.display_panel_left(ui);
                });
        }

        if self.cur_page == MenuPage::Instances {
            egui::SidePanel::right("devices_panel")
                .resizable(false)
                .exact_width(180.0)
                .show(ctx, |ui| {
                    if self.task.is_some() {
                        ui.disable();
                    }
                    self.display_panel_right(ui, ctx);
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            if self.task.is_some() {
                ui.disable();
            }
            match self.cur_page {
                MenuPage::Games => self.display_page_games(ui),
                MenuPage::Settings => self.display_page_settings(ui),
                MenuPage::Instances => self.display_page_instances(ui),
            }
        });

        // Edit handler modal
        if self.show_edit_modal {
            self.display_edit_handler_modal(ctx);
        }

        // Profile dropdown overlay
        if self.profile_dropdown_open {
            self.display_profile_dropdown(ctx);
        }

        // New profile dialog
        if self.show_new_profile_dialog {
            self.display_new_profile_dialog(ctx);
        }

        if let Some(handle) = self.task.take() {
            if handle.is_finished() {
                let _ = handle.join();
                self.loading_since = None;
                self.loading_msg = None;
            } else {
                self.task = Some(handle);
            }
        }
        if let Some(start) = self.loading_since {
            if start.elapsed() > std::time::Duration::from_secs(60) {
                // Give up waiting after one minute
                self.loading_msg = Some("Operation timed out".to_string());
            }
        }
        if let Some(msg) = &self.loading_msg {
            egui::Area::new("loading".into())
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .interactable(false)
                .show(ctx, |ui| {
                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgba_premultiplied(0, 0, 0, 192))
                        .corner_radius(6.0)
                        .inner_margin(egui::Margin::symmetric(16, 12))
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add(egui::widgets::Spinner::new().size(40.0));
                                ui.add_space(8.0);
                                ui.label(msg);
                            });
                        });
                });
        }
        if ctx.input(|input| input.focused) {
            ctx.request_repaint_after(std::time::Duration::from_millis(33)); // 30 fps
        }
    }
}

impl PartyApp {
    pub fn spawn_task<F>(&mut self, msg: &str, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.loading_msg = Some(msg.to_string());
        self.loading_since = Some(std::time::Instant::now());
        self.task = Some(std::thread::spawn(f));
    }

    pub fn is_lite(&self) -> bool {
        self.handler_lite.is_some()
    }

    /// Poll for device hotplug events and update input_devices list
    fn poll_device_events(&mut self) {
        let monitor = match &mut self.device_monitor {
            Some(m) => m,
            None => return,
        };

        for event in monitor.poll_events() {
            match event {
                DeviceEvent::Added(path) => {
                    println!("[splitux] udev: Add event for {}", path);
                    // Remove any stale entry with the same path first
                    if let Some(idx) = self.input_devices.iter().position(|d| d.path() == path) {
                        println!("[splitux] udev: Removing stale entry for {}", path);
                        // Clean up instances referencing this device
                        for instance in &mut self.instances {
                            instance.devices.retain(|&d| d != idx);
                        }
                        self.instances.retain(|i| !i.devices.is_empty());
                        for instance in &mut self.instances {
                            for dev_idx in &mut instance.devices {
                                if *dev_idx > idx {
                                    *dev_idx -= 1;
                                }
                            }
                        }
                        self.input_devices.remove(idx);
                    }
                    // Try to open the device
                    if let Some(device) = open_device(&path, &self.options.pad_filter_type) {
                        println!(
                            "[splitux] udev: Device connected: {} ({})",
                            device.fancyname(),
                            path
                        );
                        self.input_devices.push(device);
                        self.input_devices.sort_by_key(|d| d.path().to_string());
                    }
                }
                DeviceEvent::Removed(path) => {
                    // Find and remove the device
                    if let Some(idx) = self.input_devices.iter().position(|d| d.path() == path) {
                        let device = &self.input_devices[idx];
                        println!(
                            "[splitux] udev: Device disconnected: {} ({})",
                            device.fancyname(),
                            path
                        );

                        // Also remove from any instances
                        for instance in &mut self.instances {
                            instance.devices.retain(|&d| d != idx);
                        }
                        // Remove empty instances
                        self.instances.retain(|i| !i.devices.is_empty());
                        // Update device indices in instances (since we're removing one)
                        for instance in &mut self.instances {
                            for dev_idx in &mut instance.devices {
                                if *dev_idx > idx {
                                    *dev_idx -= 1;
                                }
                            }
                        }

                        self.input_devices.remove(idx);
                    }
                }
            }
        }
    }
}
