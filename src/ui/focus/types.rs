//! Focus and navigation types for gamepad UI navigation

// =============================================================================
// Page focus types
// =============================================================================

/// Pane-based focus for Games page
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum FocusPane {
    GameList,   // Left panel - game selection
    ActionBar,  // Center panel - Play, Profile, Edit buttons
    InfoPane,   // Right side - scrollable info area with buttons
}

/// Focus regions for Instances page
#[derive(Eq, PartialEq, Debug, Clone, Default)]
pub enum InstanceFocus {
    #[default]
    Devices,                              // Device panel on right side
    InstanceCard(usize, InstanceCardFocus), // Focus within instance card i
    LaunchOptions,                        // Launch options bar at bottom
    StartButton,                          // Start Game button
}

/// Focus elements within an instance card
#[derive(Eq, PartialEq, Debug, Clone, Copy, Default)]
pub enum InstanceCardFocus {
    #[default]
    Profile,        // Profile dropdown
    SetMaster,      // Set Master button
    Monitor,        // Monitor dropdown (if gamescope SDL enabled)
    InviteDevice,   // Invite Device button
    Device(usize),  // Specific device in the device list
    AudioOverride,  // Audio session override dropdown
    AudioPreference, // Audio preference dropdown (named profiles only)
}

/// Focus regions for Registry page
#[derive(Eq, PartialEq, Debug, Clone, Copy, Default)]
pub enum RegistryFocus {
    #[default]
    HandlerList,   // Left panel - handler list
    InstallButton, // Right panel - install button
}

/// Focus regions for Settings page
#[derive(Eq, PartialEq, Debug, Clone, Copy, Default)]
pub enum SettingsFocus {
    #[default]
    CategoryList,  // Left panel - category selection
    Options,       // Center panel - options for selected category
    BottomButtons, // Save/Restore buttons (in left panel)
}

/// Settings category for left panel navigation
#[derive(Eq, PartialEq, Debug, Clone, Copy, Default)]
pub enum SettingsCategory {
    #[default]
    General,
    Audio,
    Profiles,
    Controllers,
}

impl SettingsCategory {
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::General,
            1 => Self::Audio,
            2 => Self::Profiles,
            3 => Self::Controllers,
            _ => Self::General,
        }
    }

    pub fn to_index(self) -> usize {
        match self {
            Self::General => 0,
            Self::Audio => 1,
            Self::Profiles => 2,
            Self::Controllers => 3,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Audio => "Audio",
            Self::Profiles => "Profiles",
            Self::Controllers => "Controllers",
        }
    }
}

// =============================================================================
// Navigation types
// =============================================================================

/// Direction of navigation input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Unified dropdown state - tracks which dropdown is open across all pages
/// Only one dropdown can be open at a time (correct for gamepad UX)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActiveDropdown {
    /// Settings: profile controller preference (profile index)
    ProfileController(usize),
    /// Settings: profile audio preference (profile index)
    ProfileAudio(usize),
    /// Games page: profile selector (Y-button) - reserved for future use
    #[allow(dead_code)]
    GameProfile,
    /// Instances page: profile dropdown for instance
    InstanceProfile(usize),
    /// Instances page: monitor dropdown for instance
    InstanceMonitor(usize),
    /// Instances page: audio override dropdown for instance
    InstanceAudioOverride(usize),
    /// Instances page: audio preference dropdown for instance
    InstanceAudioPreference(usize),
}
