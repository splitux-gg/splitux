use std::collections::HashMap;

// =============================================================================
// Legacy types (migrated from app/app.rs, will be phased out)
// =============================================================================

/// Legacy: Pane-based focus for Games page
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub enum FocusPane {
    GameList,   // Left panel - game selection
    ActionBar,  // Center panel - Play, Profile, Edit buttons
    InfoPane,   // Right side - scrollable info area with buttons
}

/// Legacy: Focus regions for Instances page
#[derive(Eq, PartialEq, Debug, Clone, Copy, Default)]
pub enum InstanceFocus {
    #[default]
    Devices,       // Device/instance cards
    LaunchOptions, // Launch options bar at bottom
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
    Options,       // Settings options area (scrollable)
    BottomButtons, // Save/Restore buttons at bottom
}

// =============================================================================
// New navigation types (future focus system)
// =============================================================================

/// Direction of navigation input
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavDirection {
    Up,
    Down,
    Left,
    Right,
}

/// Abstracted input from raw gamepad/keyboard
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NavInput {
    Direction(NavDirection),
    Accept,
    Back,
    TabPrev,
    TabNext,
}

/// Page identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PageId {
    Games,
    Registry,
    Settings,
    Instances,
}

/// Region within a page
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId {
    pub page: PageId,
    pub region: u8,
}

/// Unique element identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FocusId {
    pub region: RegionId,
    pub element: String,
}

/// Result of processing navigation input
#[derive(Debug, Clone, PartialEq)]
pub enum NavResult {
    FocusChanged(FocusId),
    Activated(FocusId),
    Back,
    TabChanged(PageId),
    None,
}

/// Focus/navigation state
#[derive(Debug, Clone, Default)]
pub struct FocusState {
    pub current: Option<FocusId>,
    pub page: Option<PageId>,
    pub region_memory: HashMap<RegionId, String>,
}
