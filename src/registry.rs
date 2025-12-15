use crate::paths::PATH_PARTY;
use serde::Deserialize;
use std::error::Error;
use std::path::PathBuf;

pub const REGISTRY_URL: &str =
    "https://raw.githubusercontent.com/splitux-gg/splitux-handlers/main/index.json";
pub const REGISTRY_BASE: &str =
    "https://raw.githubusercontent.com/splitux-gg/splitux-handlers/main/handlers";

#[derive(Deserialize, Clone, Debug)]
pub struct RegistryIndex {
    #[allow(dead_code)]
    pub version: u32,
    pub handlers: Vec<RegistryEntry>,
}

#[derive(Deserialize, Clone, Debug)]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    pub author: String,
    #[serde(default)]
    pub steam_appid: Option<u32>,
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub updated: String,
}

impl RegistryEntry {
    pub fn yaml_url(&self) -> String {
        format!("{}/{}/handler.yaml", REGISTRY_BASE, self.id)
    }

    pub fn icon_url(&self) -> String {
        format!("{}/{}/icon.jpg", REGISTRY_BASE, self.id)
    }

    pub fn header_url(&self) -> String {
        format!("{}/{}/header.jpg", REGISTRY_BASE, self.id)
    }

    /// Check if this handler is already installed locally
    pub fn is_installed(&self) -> bool {
        let handler_dir = PATH_PARTY.join("handlers").join(&self.id);
        handler_dir.join("handler.yaml").exists()
    }

    /// Get the local installation path for this handler
    pub fn local_path(&self) -> PathBuf {
        PATH_PARTY.join("handlers").join(&self.id)
    }
}

/// Fetch the registry index from GitHub
pub fn fetch_registry() -> Result<RegistryIndex, String> {
    let client = reqwest::blocking::Client::new();
    let response = client
        .get(REGISTRY_URL)
        .header("User-Agent", "splitux")
        .send()
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        return Err(format!("Failed to fetch registry: HTTP {}", response.status()));
    }

    let index: RegistryIndex = response.json().map_err(|e| e.to_string())?;
    Ok(index)
}

/// Download a handler from the registry and install it locally
pub fn download_handler(entry: &RegistryEntry) -> Result<(), Box<dyn Error>> {
    let client = reqwest::blocking::Client::new();
    let handler_dir = entry.local_path();

    // Create handler directory
    std::fs::create_dir_all(&handler_dir)?;

    // Download handler.yaml
    let yaml_response = client
        .get(&entry.yaml_url())
        .header("User-Agent", "splitux")
        .send()?;

    if !yaml_response.status().is_success() {
        return Err(format!(
            "Failed to download handler.yaml: HTTP {}",
            yaml_response.status()
        )
        .into());
    }

    let yaml_content = yaml_response.text()?;
    std::fs::write(handler_dir.join("handler.yaml"), yaml_content)?;

    // Download icon.jpg (optional - don't fail if missing)
    if let Ok(icon_response) = client
        .get(&entry.icon_url())
        .header("User-Agent", "splitux")
        .send()
    {
        if icon_response.status().is_success() {
            if let Ok(icon_bytes) = icon_response.bytes() {
                let _ = std::fs::write(handler_dir.join("icon.jpg"), icon_bytes);
            }
        }
    }

    // Download header.jpg (optional - don't fail if missing)
    if let Ok(header_response) = client
        .get(&entry.header_url())
        .header("User-Agent", "splitux")
        .send()
    {
        if header_response.status().is_success() {
            if let Ok(header_bytes) = header_response.bytes() {
                let _ = std::fs::write(handler_dir.join("header.jpg"), header_bytes);
            }
        }
    }

    Ok(())
}

/// Remove an installed handler from the registry
#[allow(dead_code)]
pub fn uninstall_handler(entry: &RegistryEntry) -> Result<(), Box<dyn Error>> {
    let handler_dir = entry.local_path();
    if handler_dir.exists() {
        std::fs::remove_dir_all(&handler_dir)?;
    }
    Ok(())
}
