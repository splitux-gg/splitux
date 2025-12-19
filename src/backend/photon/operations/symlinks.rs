//! Shared file symlink setup
//!
//! Sets up shared files between Photon instances for mods like LocalMultiplayer.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;

use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::PATH_PARTY;

/// Set up shared files between instances
///
/// For mods like LocalMultiplayer that need to share data (e.g., lobby IDs),
/// this creates symlinks from each instance's expected file location to a
/// shared file location.
pub fn setup_shared_files(
    handler: &Handler,
    instances: &[Instance],
) -> Result<(), Box<dyn std::error::Error>> {
    // Get Photon settings from new optional field (Phase 7)
    let photon_settings = handler
        .photon_ref()
        .ok_or("Photon backend not enabled")?;

    if photon_settings.shared_files.is_empty() {
        return Ok(());
    }

    // Create shared directory for this game session
    let shared_dir = PATH_PARTY.join("tmp").join("photon-shared");
    fs::create_dir_all(&shared_dir)?;

    for shared_path_pattern in &photon_settings.shared_files {
        // Get the filename for the shared file
        let file_name = Path::new(shared_path_pattern)
            .file_name()
            .ok_or_else(|| format!("Invalid shared file path: {}", shared_path_pattern))?
            .to_string_lossy();

        let shared_file = shared_dir.join(file_name.as_ref());

        // Create initial shared file if it doesn't exist
        // For GlobalSave, pre-populate with spoofed accounts using profile names
        if !shared_file.exists() {
            let initial_content = if file_name == "GlobalSave" {
                // Generate spoofed accounts from instances (skip first - that's the host)
                // The LocalMultiplayer mod picks from this list when joining
                let accounts: Vec<String> = instances
                    .iter()
                    .skip(1) // Skip host (instance 0)
                    .enumerate()
                    .map(|(i, inst)| {
                        format!(
                            r#"    {{"Username": "{}", "SteamId": {}}}"#,
                            inst.profname,
                            76561198000000002u64 + i as u64
                        )
                    })
                    .collect();

                format!(
                    r#"{{
  "SpoofSteamAccounts": [
{}
  ],
  "SpoofSteamAccountsInUse": []
}}"#,
                    accounts.join(",\n")
                )
            } else {
                // Empty file for unknown shared files
                String::new()
            };
            fs::write(&shared_file, &initial_content)?;
            println!(
                "[splitux] Created shared file: {}",
                shared_file.display()
            );
        }

        // Symlink from each instance's expected path to the shared file
        for instance in instances {
            let profile_path = PATH_PARTY.join("profiles").join(&instance.profname);
            let instance_file = profile_path.join("windata").join(shared_path_pattern);

            // Create parent directories
            if let Some(parent) = instance_file.parent() {
                fs::create_dir_all(parent)?;
            }

            // Remove existing file/symlink
            if instance_file.exists() || instance_file.is_symlink() {
                fs::remove_file(&instance_file)?;
            }

            // Create symlink to shared file
            symlink(&shared_file, &instance_file)?;
            println!(
                "[splitux] {} -> {}",
                instance_file.display(),
                shared_file.display()
            );
        }
    }

    Ok(())
}
