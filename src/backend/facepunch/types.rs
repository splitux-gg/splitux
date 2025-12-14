//! Facepunch backend type definitions

/// Configuration for a Facepunch instance
#[derive(Debug, Clone)]
pub struct FacepunchConfig {
    /// Instance index (0-based)
    pub player_index: usize,
    /// Player name for this instance
    pub account_name: String,
    /// Spoofed Steam ID
    pub steam_id: u64,
}

impl FacepunchConfig {
    /// Create a new Facepunch config
    pub fn new(player_index: usize, account_name: String, steam_id: u64) -> Self {
        Self {
            player_index,
            account_name,
            steam_id,
        }
    }
}

/// Runtime patch specification for game-specific classes
/// Used by SplituxFacepunch to apply Harmony patches at runtime
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct RuntimePatch {
    /// Target class name (e.g., "SteamManager", "GameManager")
    pub class: String,

    /// Method name to patch (mutually exclusive with property)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub method: String,

    /// Property name to patch (mutually exclusive with method)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub property: String,

    /// Action to apply from PatchActions library
    /// Available: force_true, force_false, skip, force_steam_loaded, fake_auth_ticket, photon_auth_none, log_call
    pub action: String,
}
