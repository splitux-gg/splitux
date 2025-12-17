// Core app structure and main update loop

use std::collections::HashMap;

use super::config::*;
use super::focus::FocusManager;
use crate::audio::{resolve_audio_system, scan_sinks, AudioSink, AudioSystem};
use crate::handler::*;
use crate::input::*;
use crate::instance::*;
use crate::monitor::Monitor;
use crate::profiles::*;
use crate::registry::RegistryIndex;
use crate::util::*;

use eframe::egui;

// Re-export types from ui module (migrated)
pub use crate::ui::{FocusPane, InstanceFocus, MenuPage, RegistryFocus, SettingsFocus};

pub struct Splitux {
    pub installed_steamapps: Vec<Option<steamlocate::App>>,
    pub needs_update: bool,
    pub options: PartyConfig,
    pub cur_page: MenuPage,
    pub infotext: String,

    pub monitors: Vec<Monitor>,
    pub input_devices: Vec<InputDevice>,
    pub device_display_names: Vec<String>, // Display names with duplicate suffixes
    pub device_monitor: Option<DeviceMonitor>,
    pub permission_status: crate::input::PermissionStatus, // Input device permission check
    pub permission_banner_dismissed: bool, // User dismissed the permission warning
    pub instances: Vec<Instance>,
    pub instance_add_dev: Option<usize>,
    pub instance_focus: InstanceFocus,
    pub launch_option_index: usize, // 0=Split style, 1=KB/Mouse support
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
    pub info_pane_index: usize,  // Index of focused element in info pane
    pub info_pane_scroll: f32,   // Scroll offset for info pane
    pub game_panel_bottom_focused: bool, // True if focused on Add Game/Import Handler
    pub game_panel_bottom_index: usize,  // 0=Add Game, 1=Import Handler

    // Profile dropdown state (opened with Y button)
    pub profile_dropdown_open: bool,
    pub profile_dropdown_selection: usize, // Temporary selection while dropdown is open
    pub show_new_profile_dialog: bool,

    pub loading_msg: Option<String>,
    pub loading_since: Option<std::time::Instant>,
    #[allow(dead_code)]
    pub task: Option<std::thread::JoinHandle<()>>,

    // Registry state
    pub registry_index: Option<RegistryIndex>,
    pub registry_loading: bool,
    pub registry_error: Option<String>,
    pub registry_search: String,
    pub registry_selected: Option<usize>,
    pub registry_installing: Option<String>,
    pub registry_focus: RegistryFocus,

    // Settings state
    pub settings_focus: SettingsFocus,
    pub settings_button_index: usize, // 0=Save, 1=Restore
    pub settings_option_index: usize, // Index of focused option in settings
    pub settings_scroll_to_focus: bool, // Set true when focus changes to trigger scroll

    // Audio state
    pub audio_system: AudioSystem,
    pub audio_devices: Vec<AudioSink>,

    // Profile preferences state
    /// Previous profile selections per instance (for detecting changes)
    pub prev_profile_selections: Vec<usize>,
    /// Warnings for profiles with missing preferred controllers
    pub controller_warnings: Vec<String>,
    /// Audio preferences from profiles (instance index -> sink name)
    pub profile_audio_prefs: HashMap<usize, String>,

    /// Session-only audio overrides (instance index -> sink name or None for mute)
    /// These do NOT persist to profile preferences, only apply to current launch
    pub audio_session_overrides: HashMap<usize, Option<String>>,

    // Profile management state (Settings page)
    /// Index of profile being edited/renamed (None = not editing)
    pub profile_edit_index: Option<usize>,
    /// Text buffer for profile rename operation
    pub profile_rename_buffer: String,
    /// Show confirmation dialog for profile deletion
    pub profile_delete_confirm: Option<usize>,
    /// Which profile has preferences expanded (None = all collapsed)
    pub profile_prefs_expanded: Option<usize>,
    /// Sub-focus within expanded profile (0 = header/toggle, 1 = controller, 2 = audio)
    pub profile_prefs_focus: usize,
    /// Which profile's controller combo is forced open (by gamepad A press)
    pub profile_ctrl_combo_open: Option<usize>,
    /// Which profile's audio combo is forced open (by gamepad A press)
    pub profile_audio_combo_open: Option<usize>,
    /// Selected index within open dropdown (0 = None, 1+ = devices)
    pub profile_dropdown_selection_idx: usize,

    // Device naming state
    /// Index of device being renamed (None = not renaming)
    pub device_rename_index: Option<usize>,
    /// Text buffer for device rename operation
    pub device_rename_buffer: String,

    // Panel collapse/resize state
    pub games_panel_collapsed: bool,
    pub games_panel_width: f32,
    pub devices_panel_collapsed: bool,
    pub devices_panel_width: f32,
}

impl Splitux {
    pub fn new(monitors: Vec<Monitor>, handler_lite: Option<Handler>) -> Self {
        let options = load_cfg();
        let input_devices = scan_input_devices(&options.pad_filter_type);
        let device_display_names =
            crate::input::generate_display_names(&input_devices, &options.device_aliases);

        // Extract panel layout state before options is moved
        let games_panel_collapsed = options.layout.games_panel.collapsed;
        let games_panel_width = options.layout.games_panel.custom_width.unwrap_or(160.0);
        let devices_panel_collapsed = options.layout.devices_panel.collapsed;
        let devices_panel_width = options.layout.devices_panel.custom_width.unwrap_or(200.0);
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

        // Scan audio devices
        let audio_system = resolve_audio_system(options.audio.system);
        let audio_devices = if audio_system != AudioSystem::None {
            scan_sinks(audio_system).unwrap_or_else(|e| {
                eprintln!("[splitux] audio: Failed to scan audio devices: {}", e);
                Vec::new()
            })
        } else {
            Vec::new()
        };
        if !audio_devices.is_empty() {
            println!("[splitux] audio: Found {} audio output devices", audio_devices.len());
        }

        let mut app = Self {
            installed_steamapps: get_installed_steamapps(),
            needs_update: false,
            options,
            cur_page,
            infotext: String::new(),
            monitors,
            input_devices,
            device_display_names,
            device_monitor,
            permission_status: crate::input::check_permissions(),
            permission_banner_dismissed: false,
            instances: Vec::new(),
            instance_add_dev: None,
            instance_focus: InstanceFocus::default(),
            launch_option_index: 0,
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
            info_pane_index: 0,
            info_pane_scroll: 0.0,
            game_panel_bottom_focused: false,
            game_panel_bottom_index: 0,
            profile_dropdown_open: false,
            profile_dropdown_selection: 0,
            show_new_profile_dialog: false,
            loading_msg: None,
            loading_since: None,
            task: None,

            // Registry state
            registry_index: None,
            registry_loading: false,
            registry_error: None,
            registry_search: String::new(),
            registry_selected: None,
            registry_installing: None,
            registry_focus: RegistryFocus::default(),

            // Settings state
            settings_focus: SettingsFocus::default(),
            settings_button_index: 0,
            settings_option_index: 0,
            settings_scroll_to_focus: false,

            // Audio state
            audio_system,
            audio_devices,

            // Profile preferences state
            prev_profile_selections: Vec::new(),
            controller_warnings: Vec::new(),
            profile_audio_prefs: HashMap::new(),
            audio_session_overrides: HashMap::new(),

            // Profile management state
            profile_edit_index: None,
            profile_rename_buffer: String::new(),
            profile_delete_confirm: None,
            profile_prefs_expanded: None,
            profile_prefs_focus: 0,
            profile_ctrl_combo_open: None,
            profile_audio_combo_open: None,
            profile_dropdown_selection_idx: 0,

            // Device naming state
            device_rename_index: None,
            device_rename_buffer: String::new(),

            // Panel collapse/resize state (loaded from config above)
            games_panel_collapsed,
            games_panel_width,
            devices_panel_collapsed,
            devices_panel_width,
        };

        app.spawn_task("Checking for updates", move || {
            app.needs_update = check_for_splitux_update();
        });

        app
    }
}

impl eframe::App for Splitux {
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
        // Paint full-screen background to fill any gaps between panels
        let screen_rect = ctx.screen_rect();
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(screen_rect, 0.0, super::theme::colors::BG_DARK);

        // Poll for device hotplug events
        self.poll_device_events();

        // Reset focus state at start of frame
        self.focus_manager.begin_frame();

        // Enable keyboard focus navigation
        ctx.options_mut(|opt| {
            opt.input_options.line_scroll_speed = 40.0;
        });

        egui::TopBottomPanel::top("menu_nav_panel")
            .frame(egui::Frame::NONE
                .fill(super::theme::colors::BG_MID)
                .inner_margin(egui::Margin::symmetric(8, 4)))
            .show(ctx, |ui| {
                if self.task.is_some() {
                    ui.disable();
                }
                self.display_panel_top(ui);
            });

        // Left panel - Games list (collapsible/resizable)
        if !self.is_lite() {
            let collapsed = self.games_panel_collapsed;
            let (width, width_range) = if collapsed {
                (36.0, 36.0..=36.0)
            } else {
                (self.games_panel_width, 120.0..=280.0)
            };

            egui::SidePanel::left("games_panel")
                .resizable(!collapsed)
                .default_width(width)
                .width_range(width_range)
                .frame(egui::Frame::NONE
                    .fill(super::theme::colors::BG_MID)
                    .inner_margin(if collapsed {
                        egui::Margin::symmetric(4, 8)
                    } else {
                        egui::Margin::same(8)
                    })
                    .stroke(egui::Stroke::new(1.0, super::theme::colors::BG_LIGHT)))
                .show_separator_line(false)
                .show(ctx, |ui| {
                    if self.task.is_some() {
                        ui.disable();
                    }
                    if collapsed {
                        self.display_collapsed_games_panel(ui);
                    } else {
                        // Track width changes for persistence
                        let panel_width = ui.available_width() + 16.0; // Account for margins
                        if (panel_width - self.games_panel_width).abs() > 2.0 {
                            self.games_panel_width = panel_width;
                        }
                        self.display_panel_left(ui);
                    }
                });
        }

        // Right panel - Devices (collapsible/resizable, only on Instances page)
        if self.cur_page == MenuPage::Instances {
            let collapsed = self.devices_panel_collapsed;
            let (width, width_range) = if collapsed {
                (36.0, 36.0..=36.0)
            } else {
                (self.devices_panel_width, 150.0..=320.0)
            };

            egui::SidePanel::right("devices_panel")
                .resizable(!collapsed)
                .default_width(width)
                .width_range(width_range)
                .frame(egui::Frame::NONE
                    .fill(super::theme::colors::BG_MID)
                    .inner_margin(if collapsed {
                        egui::Margin::symmetric(4, 8)
                    } else {
                        egui::Margin { left: 16, right: 8, top: 8, bottom: 8 }
                    })
                    .stroke(egui::Stroke::new(1.0, super::theme::colors::BG_LIGHT)))
                .show_separator_line(false)
                .show(ctx, |ui| {
                    if self.task.is_some() {
                        ui.disable();
                    }
                    if collapsed {
                        self.display_collapsed_devices_panel(ui);
                    } else {
                        // Track width changes for persistence
                        let panel_width = ui.available_width() + 24.0; // Account for margins
                        if (panel_width - self.devices_panel_width).abs() > 2.0 {
                            self.devices_panel_width = panel_width;
                        }
                        self.display_panel_right(ui, ctx);
                    }
                });
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE
                .fill(super::theme::colors::BG_DARK)
                .inner_margin(egui::Margin { left: 8, right: 8, top: 0, bottom: 8 }))
            .show(ctx, |ui| {
            if self.task.is_some() {
                ui.disable();
            }

            // Show permission banner at top if needed (only on Games/Instances pages)
            if matches!(self.cur_page, MenuPage::Games | MenuPage::Instances) {
                ui.add_space(8.0);
                self.display_permission_banner(ui);
            }

            match self.cur_page {
                MenuPage::Games => self.display_page_games(ui),
                MenuPage::Registry => self.display_page_registry(ui),
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

impl Splitux {
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
                        self.refresh_device_display_names();
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
                        self.refresh_device_display_names();
                    }
                }
            }
        }
    }

    /// Regenerate display names for all input devices (handles duplicates)
    pub fn refresh_device_display_names(&mut self) {
        self.device_display_names =
            crate::input::generate_display_names(&self.input_devices, &self.options.device_aliases);
    }

    /// Get display name for a device by index
    pub fn device_display_name(&self, idx: usize) -> &str {
        self.device_display_names
            .get(idx)
            .map(|s| s.as_str())
            .unwrap_or_else(|| self.input_devices.get(idx).map(|d| d.fancyname()).unwrap_or("Unknown"))
    }

    /// Show permission warning banner if needed, returns true if banner was shown
    pub fn display_permission_banner(&mut self, ui: &mut egui::Ui) -> bool {
        // Don't show if dismissed or no permission issues
        if self.permission_banner_dismissed || self.permission_status.denied_count == 0 {
            return false;
        }

        use egui::RichText;

        let banner_color = egui::Color32::from_rgb(180, 120, 40); // Orange/amber warning
        egui::Frame::NONE
            .fill(banner_color.gamma_multiply(0.3))
            .stroke(egui::Stroke::new(1.0, banner_color))
            .rounding(4.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("⚠").size(18.0).color(banner_color));
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Controller access requires setup")
                                .strong()
                                .color(egui::Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} input device(s) not accessible. Click 'Fix' to install udev rules.",
                                self.permission_status.denied_count
                            ))
                            .small()
                            .color(egui::Color32::LIGHT_GRAY),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Dismiss button (use simple X for font compatibility)
                        if ui.small_button("X").on_hover_text("Dismiss").clicked() {
                            self.permission_banner_dismissed = true;
                        }

                        ui.add_space(8.0);

                        // Fix button - installs udev rules via pkexec
                        let fix_btn = ui.button("Fix Permissions");
                        if fix_btn.on_hover_text("Install udev rules (requires password)").clicked() {
                            println!("[splitux] Attempting to install udev rules via pkexec...");
                            match crate::input::install_udev_rules() {
                                Ok(true) => {
                                    println!("[splitux] Udev rules installed successfully");
                                    // Refresh permission status
                                    self.permission_status = crate::input::check_permissions();
                                    self.infotext = "Udev rules installed! Reconnect your controllers.".to_string();
                                }
                                Ok(false) => {
                                    println!("[splitux] User cancelled pkexec dialog");
                                    self.infotext = "Installation cancelled.".to_string();
                                }
                                Err(e) => {
                                    println!("[splitux] Failed to install udev rules: {}", e);
                                    self.infotext = format!("Failed: {}", e);
                                }
                            }
                        }
                    });
                });
            });

        ui.add_space(8.0);
        true
    }
}
