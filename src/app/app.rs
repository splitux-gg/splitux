//! Core app structure and main update loop
//!
//! This module is split into submodules:
//! - `eframe_impl` - eframe::App implementation
//! - `helpers` - Utility methods (task spawning, device events, display names)

mod eframe_impl;
mod helpers;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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

// Re-export types from ui module (migrated)
pub use crate::ui::{ActiveDropdown, FocusPane, InstanceFocus, MenuPage, ProfileBuilderFocus, RegistryFocus, SettingsCategory, SettingsFocus};

pub struct Splitux {
    pub installed_steamapps: Vec<Option<steamlocate::App>>,
    pub needs_update: Arc<AtomicBool>,
    pub options: SplituxConfig,
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
    pub settings_category: SettingsCategory,     // Selected category in left panel
    pub settings_panel_collapsed: bool,          // Left panel collapsed state
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
    /// Warnings for profiles with missing preferred audio devices
    pub audio_warnings: Vec<String>,
    /// Audio preferences from profiles (instance index -> sink name)
    pub profile_audio_prefs: HashMap<usize, String>,

    /// Session-only audio overrides (instance index -> sink name or None for mute)
    /// These do NOT persist to profile preferences, only apply to current launch
    pub audio_session_overrides: HashMap<usize, Option<String>>,

    /// Session-only gptokeyb profile overrides (instance index -> profile name)
    /// None means use handler's default, Some("") means disabled
    pub gptokeyb_instance_overrides: HashMap<usize, String>,

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
    /// Which dropdown is currently open (unified for all pages)
    pub active_dropdown: Option<ActiveDropdown>,
    /// Selected index within open dropdown (0 = None, 1+ = devices)
    pub dropdown_selection_idx: usize,

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

    // Monitor polling state
    pub last_monitor_poll: std::time::Instant,

    // Layout customization state
    pub layout_custom_mode: bool,        // True when in custom assignment mode
    pub layout_focused_region: usize,    // Which region is currently focused
    pub layout_edit_order: Vec<usize>,   // Live editing buffer for instance order

    // Profile Builder state (KB/Mouse Mapper)
    pub profile_builder_profiles: Vec<String>,    // List of user-created gptokeyb profiles
    pub profile_builder_editing: Option<crate::gptokeyb::GptokeybProfile>, // Profile being edited
    pub profile_builder_selected_button: Option<crate::gptokeyb::ControllerButton>, // Button selected for mapping
    pub profile_builder_name_buffer: String,      // Name input buffer for new profiles
    pub profile_builder_focus: ProfileBuilderFocus, // Gamepad navigation focus
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
        let devices_panel_collapsed = true; // Always start collapsed
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

        let profiles = scan_profiles(true);

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
            println!(
                "[splitux] audio: Found {} audio output devices",
                audio_devices.len()
            );
        }

        let app = Self {
            installed_steamapps: get_installed_steamapps(),
            needs_update: Arc::new(AtomicBool::new(false)),
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
            settings_category: SettingsCategory::default(),
            settings_panel_collapsed: false,
            settings_button_index: 0,
            settings_option_index: 0,
            settings_scroll_to_focus: false,

            // Audio state
            audio_system,
            audio_devices,

            // Profile preferences state
            prev_profile_selections: Vec::new(),
            controller_warnings: Vec::new(),
            audio_warnings: Vec::new(),
            profile_audio_prefs: HashMap::new(),
            audio_session_overrides: HashMap::new(),
            gptokeyb_instance_overrides: HashMap::new(),

            // Profile management state
            profile_edit_index: None,
            profile_rename_buffer: String::new(),
            profile_delete_confirm: None,
            profile_prefs_expanded: None,
            profile_prefs_focus: 0,
            active_dropdown: None,
            dropdown_selection_idx: 0,

            // Device naming state
            device_rename_index: None,
            device_rename_buffer: String::new(),

            // Panel collapse/resize state (loaded from config above)
            games_panel_collapsed,
            games_panel_width,
            devices_panel_collapsed,
            devices_panel_width,

            // Monitor polling state
            last_monitor_poll: std::time::Instant::now(),

            // Layout customization state
            layout_custom_mode: false,
            layout_focused_region: 0,
            layout_edit_order: Vec::new(),

            // Profile Builder state
            profile_builder_profiles: crate::gptokeyb::list_user_profiles(),
            profile_builder_editing: None,
            profile_builder_selected_button: None,
            profile_builder_name_buffer: String::new(),
            profile_builder_focus: ProfileBuilderFocus::default(),
        };

        let needs_update = app.needs_update.clone();
        std::thread::spawn(move || {
            if check_for_splitux_update() {
                needs_update.store(true, Ordering::Relaxed);
            }
        });

        app
    }
}
