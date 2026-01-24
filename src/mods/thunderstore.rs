//! Thunderstore mod repository client
//!
//! Handles downloading and caching plugins from Thunderstore.

use super::types::PluginSource;
use serde::Deserialize;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

/// Thunderstore package version info from API
#[derive(Debug, Deserialize)]
struct PackageVersion {
    version_number: String,
}

/// Thunderstore package info from API (experimental endpoint)
#[derive(Debug, Deserialize)]
struct PackageInfo {
    latest: Option<PackageVersion>,
}

/// Fetch a plugin from Thunderstore, using cache if available.
/// Returns list of all extracted file paths.
pub fn fetch_plugin(source: &PluginSource, cache_base: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let cache_dir = source.cache_path(cache_base);

    // Check if already cached
    if cache_dir.exists() {
        eprintln!("[mods] Using cached plugin: {}", source.display_name());
        return collect_files(&cache_dir);
    }

    // Download and extract
    download_and_extract(source, &cache_dir)?;
    collect_files(&cache_dir)
}

/// Download plugin archive and extract to cache directory
fn download_and_extract(source: &PluginSource, cache_dir: &Path) -> Result<(), Box<dyn Error>> {
    let url = source.thunderstore_url();
    eprintln!("[mods] Downloading plugin from: {}", url);

    // Create cache directory
    fs::create_dir_all(cache_dir)?;

    // Download to temp file
    let zip_path = cache_dir.join("plugin.zip");
    download_file(&url, &zip_path)?;

    // Extract
    extract_zip(&zip_path, cache_dir)?;

    // Remove zip after extraction
    fs::remove_file(&zip_path).ok();

    eprintln!("[mods] Plugin extracted to: {:?}", cache_dir);
    Ok(())
}

/// Download a file from URL to local path
fn download_file(url: &str, dest: &Path) -> Result<(), Box<dyn Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()?;

    let response = client.get(url).send()?;

    if !response.status().is_success() {
        return Err(format!("Download failed: HTTP {}", response.status()).into());
    }

    let bytes = response.bytes()?;
    let mut file = File::create(dest)?;
    file.write_all(&bytes)?;

    Ok(())
}

/// Extract a zip archive to destination directory
fn extract_zip(zip_path: &Path, dest_dir: &Path) -> Result<(), Box<dyn Error>> {
    let file = File::open(zip_path)?;
    let reader = BufReader::new(file);
    let mut archive = ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => dest_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(parent) = outpath.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut outfile = File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }

        // Set permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
            }
        }
    }

    Ok(())
}

/// Fetch the latest version of a package from Thunderstore API
fn fetch_latest_version(_community: &str, package: &str) -> Result<String, Box<dyn Error>> {
    // Thunderstore experimental API endpoint for package info
    // Format: https://thunderstore.io/api/experimental/package/{namespace}/{name}/
    let (namespace, name) = package
        .split_once('/')
        .ok_or_else(|| format!("Invalid package format: {}", package))?;

    let url = format!(
        "https://thunderstore.io/api/experimental/package/{}/{}/",
        namespace, name
    );

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    let response = client.get(&url).send()?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch package info: HTTP {}", response.status()).into());
    }

    let info: PackageInfo = response.json()?;

    // Get version from the "latest" field
    info.latest
        .map(|v| v.version_number)
        .ok_or_else(|| "No latest version found for package".into())
}

/// Get the currently cached version of a package, if any
fn get_cached_version(cache_base: &Path, community: &str, package: &str) -> Option<String> {
    let package_dir = cache_base
        .join("thunderstore")
        .join(community)
        .join(package.replace('/', "_"));

    if !package_dir.exists() {
        return None;
    }

    // Find version directories
    if let Ok(entries) = fs::read_dir(&package_dir) {
        for entry in entries.flatten() {
            if entry.path().is_dir() {
                if let Some(version) = entry.file_name().to_str() {
                    return Some(version.to_string());
                }
            }
        }
    }

    None
}

/// Fetch a BepInEx package from Thunderstore for a specific community.
/// Returns the path to the extracted package directory.
///
/// The `package` parameter specifies which BepInEx package to use (e.g., "bbepis/BepInExPack"
/// or "xiaoye97/BepInEx"). Different packages may have different doorstop versions.
///
/// Automatically fetches the latest version and updates if a newer version is available.
pub fn fetch_bepinex_pack(
    community: &str,
    package: &str,
    cache_base: &Path,
) -> Result<PathBuf, Box<dyn Error>> {

    // Fetch latest version from Thunderstore API
    let latest_version = match fetch_latest_version(community, package) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("[mods] Warning: Failed to check latest {} version: {}", package, e);
            // Fall back to cached version if available
            if let Some(cached) = get_cached_version(cache_base, community, package) {
                eprintln!("[mods] Using cached {} version {}", package, cached);
                let source = PluginSource {
                    source: "thunderstore".to_string(),
                    community: community.to_string(),
                    package: package.to_string(),
                    version: cached,
                };
                return Ok(source.cache_path(cache_base));
            }
            return Err(e);
        }
    };

    let source = PluginSource {
        source: "thunderstore".to_string(),
        community: community.to_string(),
        package: package.to_string(),
        version: latest_version.clone(),
    };

    let cache_dir = source.cache_path(cache_base);

    // Check if we have the latest version cached
    if cache_dir.exists() {
        eprintln!(
            "[mods] Using cached {} {} for {}",
            package, latest_version, community
        );
        return Ok(cache_dir);
    }

    // Check if we have an older version cached
    if let Some(cached_version) = get_cached_version(cache_base, community, package) {
        if cached_version != latest_version {
            eprintln!(
                "[mods] Updating {}: {} -> {}",
                package, cached_version, latest_version
            );
            // Remove old cached version
            let old_cache = cache_base
                .join("thunderstore")
                .join(community)
                .join(package.replace('/', "_"))
                .join(&cached_version);
            fs::remove_dir_all(&old_cache).ok();
        }
    }

    // Download and extract latest
    eprintln!(
        "[mods] Downloading {} {} for {} from Thunderstore...",
        package, latest_version, community
    );
    download_and_extract(&source, &cache_dir)?;

    Ok(cache_dir)
}

/// Recursively collect all files in a directory
fn collect_files(dir: &Path) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let mut files = Vec::new();
    collect_files_recursive(dir, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn Error>> {
    if !dir.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_recursive(&path, files)?;
        } else {
            files.push(path);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thunderstore_url() {
        let source = PluginSource {
            source: "thunderstore".to_string(),
            community: "repo".to_string(),
            package: "Zehs/LocalMultiplayer".to_string(),
            version: "1.4.0".to_string(),
        };

        assert_eq!(
            source.thunderstore_url(),
            "https://thunderstore.io/package/download/Zehs/LocalMultiplayer/1.4.0/"
        );
    }

    #[test]
    fn test_cache_path() {
        let source = PluginSource {
            source: "thunderstore".to_string(),
            community: "repo".to_string(),
            package: "Zehs/LocalMultiplayer".to_string(),
            version: "1.4.0".to_string(),
        };

        let base = PathBuf::from("/home/user/.cache/splitux/mods");
        let path = source.cache_path(&base);

        assert_eq!(
            path,
            PathBuf::from("/home/user/.cache/splitux/mods/thunderstore/repo/Zehs_LocalMultiplayer/1.4.0")
        );
    }
}
