//! Thunderstore mod repository client
//!
//! Handles downloading and caching plugins from Thunderstore.

use super::types::PluginSource;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufReader, Write};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

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
