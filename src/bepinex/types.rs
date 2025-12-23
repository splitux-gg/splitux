//! BepInEx type definitions

/// Unity scripting backend type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnityBackend {
    /// Mono backend (older games, has GAME_Data/Managed/*.dll)
    Mono,
    /// IL2CPP backend (newer games, has GameAssembly.dll)
    Il2Cpp,
}

impl UnityBackend {
    /// Get display name for this backend
    pub fn display_name(&self) -> &'static str {
        match self {
            UnityBackend::Mono => "Mono",
            UnityBackend::Il2Cpp => "IL2CPP",
        }
    }
}
