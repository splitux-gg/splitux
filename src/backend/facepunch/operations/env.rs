//! Linux BepInEx environment variables
//!
//! Environment setup needed for BepInEx on Linux native games.

use std::collections::HashMap;
use std::path::Path;

/// Get environment variables needed for BepInEx on Linux native games
/// Returns empty HashMap for Windows games (doorstop uses winhttp.dll injection)
pub fn get_linux_bepinex_env(game_dir: &Path) -> HashMap<String, String> {
    let mut env = HashMap::new();

    let libdoorstop = game_dir.join("libdoorstop.so");
    let preloader = game_dir.join("BepInEx/core/BepInEx.Preloader.dll");

    if libdoorstop.exists() {
        // BepInEx doorstop environment variables (from run_bepinex.sh)
        env.insert("DOORSTOP_ENABLED".to_string(), "1".to_string());
        env.insert(
            "DOORSTOP_TARGET_ASSEMBLY".to_string(),
            preloader.to_string_lossy().to_string(),
        );

        // LD_LIBRARY_PATH must include the game directory for libdoorstop.so to be found
        env.insert(
            "LD_LIBRARY_PATH".to_string(),
            game_dir.to_string_lossy().to_string(),
        );

        // LD_PRELOAD with full path to libdoorstop.so
        env.insert(
            "LD_PRELOAD".to_string(),
            libdoorstop.to_string_lossy().to_string(),
        );
    }

    env
}
