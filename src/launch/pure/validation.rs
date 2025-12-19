//! Launch validation functions (pure, no side effects)

use crate::paths::PATH_STEAM;

/// Validate that the Steam Runtime is available
pub fn validate_runtime(runtime: &str) -> Result<(), Box<dyn std::error::Error>> {
    match runtime {
        "scout" => {
            let path = PATH_STEAM.join("ubuntu12_32/steam-runtime/run.sh");
            if !path.exists() {
                return Err(format!("Steam Runtime scout not found at {}", path.display()).into());
            }
        }
        "soldier" => {
            let path = PATH_STEAM.join("steamapps/common/SteamLinuxRuntime_soldier");
            if !path.exists() {
                return Err(format!("Steam Runtime soldier not found at {}", path.display()).into());
            }
        }
        "" => {} // No runtime specified, that's fine
        _ => {} // Unknown runtime, let it pass
    }
    Ok(())
}
