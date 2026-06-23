// Shared overlay directory setup for backends

use std::fs;
use std::path::PathBuf;

/// Prepare a clean overlay directory for a backend instance.
///
/// Creates `{PATH_PARTY}/tmp/{launch_ns}/{backend_name}-overlay-{instance_idx}`
/// (per-launch namespaced so concurrent sessions don't collide), removing any
/// previous overlay at the same path.
pub fn prepare_overlay_dir(
    backend_name: &str,
    instance_idx: usize,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let overlay_dir = crate::paths::launch_tmp_dir()
        .join(format!("{}-overlay-{}", backend_name, instance_idx));

    // Clean previous overlay
    if overlay_dir.exists() {
        fs::remove_dir_all(&overlay_dir)?;
    }
    fs::create_dir_all(&overlay_dir)?;

    Ok(overlay_dir)
}
