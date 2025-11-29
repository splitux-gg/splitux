use crate::backend::MultiplayerBackend;
use crate::paths::*;
use crate::util::*;

use eframe::egui::{self, ImageSource};
use rfd::FileDialog;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

pub const HANDLER_SPEC_CURRENT_VERSION: u16 = 3;

/// Photon-specific settings for BepInEx/LocalMultiplayer
#[derive(Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct PhotonSettings {
    /// Path pattern for LocalMultiplayer config file within profile's windata
    /// Example: "AppData/LocalLow/CompanyName/GameName/LocalMultiplayer/global.cfg"
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub config_path: String,
}

impl PhotonSettings {
    pub fn is_empty(&self) -> bool {
        self.config_path.is_empty()
    }
}

fn is_default_backend(b: &MultiplayerBackend) -> bool {
    *b == MultiplayerBackend::None
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum SDL2Override {
    #[default]
    No,
    Srt,
    Sys,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Handler {
    // Members that are determined by context (not serialized)
    #[serde(skip)]
    pub path_handler: PathBuf,
    #[serde(skip)]
    pub img_paths: Vec<PathBuf>,

    // Required fields
    pub name: String,
    pub exec: String,

    // Optional metadata
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub info: String,
    #[serde(default, skip_serializing_if = "is_default_spec_ver")]
    pub spec_ver: u16,

    // Game location (one of these should be set)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steam_appid: Option<u32>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub path_gameroot: String,

    // Launch configuration
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub runtime: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub args: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub env: String,
    #[serde(default, skip_serializing_if = "is_default_sdl2")]
    pub sdl2_override: SDL2Override,
    /// Path to Proton installation. If set, uses direct Proton instead of umu-run.
    /// Example: "Proton - Experimental" or full path like "/path/to/proton"
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub proton_path: String,

    // Multiplayer settings
    /// Multiplayer backend to use (none, goldberg, photon)
    #[serde(default, skip_serializing_if = "is_default_backend")]
    pub backend: MultiplayerBackend,
    /// DEPRECATED: Use `backend: goldberg` instead. Kept for backwards compatibility.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub use_goldberg: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pause_between_starts: Option<f64>,
    /// Game-specific Goldberg settings files.
    /// Keys are filenames (e.g., "force_lobby_type.txt", "invite_all.txt")
    /// Values are file contents (use empty string for empty files)
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub goldberg_settings: HashMap<String, String>,
    /// Photon-specific settings (only used when backend = photon)
    #[serde(default, skip_serializing_if = "PhotonSettings::is_empty")]
    pub photon_settings: PhotonSettings,

    // Advanced
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub game_null_paths: Vec<String>,
}

fn is_default_spec_ver(v: &u16) -> bool {
    *v == HANDLER_SPEC_CURRENT_VERSION || *v == 0
}

fn is_default_sdl2(v: &SDL2Override) -> bool {
    *v == SDL2Override::No
}

impl Default for Handler {
    fn default() -> Self {
        Self {
            path_handler: PathBuf::new(),
            img_paths: Vec::new(),
            path_gameroot: String::new(),

            name: String::new(),
            author: String::new(),
            version: String::new(),
            info: String::new(),
            spec_ver: HANDLER_SPEC_CURRENT_VERSION,

            runtime: String::new(),
            exec: String::new(),
            args: String::new(),
            env: String::new(),
            sdl2_override: SDL2Override::No,
            proton_path: String::new(),

            pause_between_starts: None,

            backend: MultiplayerBackend::None,
            use_goldberg: false,
            steam_appid: None,
            goldberg_settings: std::collections::HashMap::new(),
            photon_settings: PhotonSettings::default(),

            game_null_paths: Vec::new(),
        }
    }
}

impl Handler {
    pub fn from_yaml(yaml_path: &PathBuf) -> Result<Self, Box<dyn Error>> {
        let file = File::open(yaml_path)?;
        let mut handler: Handler = serde_yaml::from_reader(BufReader::new(file))?;

        handler.path_handler = yaml_path
            .parent()
            .ok_or_else(|| "Invalid path")?
            .to_path_buf();
        handler.img_paths = handler.get_imgs();

        // Clean up whitespace from all fields
        handler.trim_fields();

        // Sanitize paths
        for path in &mut handler.game_null_paths {
            *path = path.sanitize_path();
        }

        // Migrate deprecated use_goldberg to backend field
        if handler.use_goldberg && handler.backend == MultiplayerBackend::None {
            handler.backend = MultiplayerBackend::Goldberg;
            handler.use_goldberg = false; // Clear deprecated field
        }

        // Validate required fields
        handler.validate()?;

        Ok(handler)
    }

    /// Trim whitespace from all string fields
    fn trim_fields(&mut self) {
        self.name = self.name.trim().to_string();
        self.exec = self.exec.trim().to_string();
        self.author = self.author.trim().to_string();
        self.version = self.version.trim().to_string();
        self.info = self.info.trim().to_string();
        self.path_gameroot = self.path_gameroot.trim().to_string();
        self.runtime = self.runtime.trim().to_string();
        self.args = self.args.trim().to_string();
        self.env = self.env.trim().to_string();
        self.proton_path = self.proton_path.trim().to_string();

        // Trim paths in null_paths list
        for path in &mut self.game_null_paths {
            *path = path.trim().to_string();
        }
        // Remove empty entries
        self.game_null_paths.retain(|p| !p.is_empty());
    }

    /// Validate that required fields are present
    fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.name.is_empty() {
            return Err("Handler 'name' is required".into());
        }
        if self.exec.is_empty() {
            return Err("Handler 'exec' (executable path) is required".into());
        }
        Ok(())
    }

    pub fn from_cli(path_exec: &str, args: &str) -> Self {
        let mut handler = Self::default();

        handler.path_gameroot = Path::new(path_exec)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap();
        handler.exec = Path::new(path_exec)
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap();
        handler.args = args.to_string();

        handler
    }

    pub fn icon(&self) -> ImageSource<'_> {
        if self.path_handler.join("icon.png").exists() {
            format!("file://{}/icon.png", self.path_handler.display()).into()
        } else {
            egui::include_image!("../res/executable_icon.png")
        }
    }

    pub fn display(&self) -> &str {
        self.name.as_str()
    }

    pub fn display_clamp(&self) -> String {
        if self.name.len() > 25 {
            let out = format!("{}...", &self.name[..22]);
            out.clone()
        } else {
            self.name.clone()
        }
    }

    pub fn win(&self) -> bool {
        self.exec.ends_with(".exe") || self.exec.ends_with(".bat")
    }

    pub fn is_saved_handler(&self) -> bool {
        !self.path_handler.as_os_str().is_empty()
    }

    pub fn handler_dir_name(&self) -> &str {
        self.path_handler
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    }

    fn get_imgs(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let imgs_path = self.path_handler.join("imgs");

        let entries = match std::fs::read_dir(imgs_path) {
            Ok(entries) => entries,
            Err(_) => return out,
        };

        for entry_result in entries {
            if let Ok(entry) = entry_result
                && let Ok(file_type) = entry.file_type()
                && file_type.is_file()
                && let Some(path_str) = entry.path().to_str()
                && (path_str.ends_with(".png") || path_str.ends_with(".jpg"))
            {
                out.push(entry.path());
            }
        }

        out.sort();
        out
    }

    pub fn remove_handler(&self) -> Result<(), Box<dyn Error>> {
        if !self.is_saved_handler() {
            return Err("No handler directory to remove".into());
        }
        // TODO: Also return err if handler path exists but is not inside PATH_PARTY/handlers
        std::fs::remove_dir_all(self.path_handler.clone())?;

        Ok(())
    }

    pub fn get_game_rootpath(&self) -> Result<String, Box<dyn Error>> {
        if let Some(appid) = &self.steam_appid
            && let Some((app, library)) = steamlocate::SteamDir::locate()?
                .find_app(*appid)
                .ok()
                .flatten()
        {
            {
                let path = library.resolve_app_dir(&app);
                if path.exists() {
                    let pathstr = path.to_string_lossy().to_string();
                    return Ok(pathstr);
                }
            }
        }

        if !self.path_gameroot.is_empty() && Path::new(&self.path_gameroot).exists() {
            return Ok(self.path_gameroot.clone());
        }

        Err("Game root path not found".into())
    }

    pub fn save(&mut self) -> Result<(), Box<dyn Error>> {
        // If handler has no path, assume we're saving a newly created handler
        if !self.is_saved_handler() {
            if self.name.is_empty() {
                // If handler is based on a Steam game try to get the game's install dir name
                if let Some(appid) = self.steam_appid
                    && let Ok(dir) = steamlocate::SteamDir::locate()
                    && let Ok(Some((app, _))) = dir.find_app(appid)
                {
                    self.name = app.install_dir;
                } else {
                    return Err("Name cannot be empty".into());
                }
            }
            if !PATH_PARTY.join("handlers").join(&self.name).exists() {
                self.path_handler = PATH_PARTY.join("handlers").join(&self.name);
            } else {
                let mut i = 1;
                while PATH_PARTY
                    .join("handlers")
                    .join(&format!("{}-{}", self.name, i))
                    .exists()
                {
                    i += 1;
                }
                self.path_handler = PATH_PARTY
                    .join("handlers")
                    .join(&format!("{}-{}", self.name, i));
            }
        }

        if !self.path_handler.exists() {
            std::fs::create_dir_all(&self.path_handler)?;
        }

        let yaml = serde_yaml::to_string(self)?;
        std::fs::write(self.path_handler.join("handler.yaml"), yaml)?;

        Ok(())
    }

    pub fn export(&self) -> Result<(), Box<dyn Error>> {
        if self.name.is_empty() {
            return Err("Name cannot be empty".into());
        }

        let mut file = FileDialog::new()
            .set_title("Save file to:")
            .set_directory(&*PATH_HOME)
            .add_filter("Splitux Handler Package", &["spx"])
            .save_file()
            .ok_or_else(|| "File not specified")?;

        if file.extension().is_none() || file.extension() != Some("spx".as_ref()) {
            file.set_extension("spx");
        }

        let tmpdir = PATH_PARTY.join("tmp");
        std::fs::create_dir_all(&tmpdir)?;

        copy_dir_recursive(&self.path_handler, &tmpdir)?;

        // Clear the rootpath before exporting so that users downloading it can set their own
        let mut handlerclone = self.clone();
        handlerclone.path_gameroot = String::new();
        // Overwrite the handler.yaml file with handlerclone
        let yaml = serde_yaml::to_string(&handlerclone)?;
        std::fs::write(tmpdir.join("handler.yaml"), yaml)?;

        if file.is_file() {
            std::fs::remove_file(&file)?;
        }

        zip_dir(&tmpdir, &file)?;
        clear_tmp()?;

        Ok(())
    }
}

pub fn scan_handlers() -> Vec<Handler> {
    let mut out: Vec<Handler> = Vec::new();
    let handlers_path = PATH_PARTY.join("handlers");

    let Ok(entries) = std::fs::read_dir(handlers_path) else {
        return out;
    };

    for entry_result in entries {
        if let Ok(entry) = entry_result
            && let Ok(file_type) = entry.file_type()
            && file_type.is_dir()
        {
            let yaml_path = entry.path().join("handler.yaml");
            if yaml_path.exists()
                && let Ok(handler) = Handler::from_yaml(&yaml_path)
            {
                out.push(handler);
            }
        }
    }
    out.sort_by(|a, b| a.display().to_lowercase().cmp(&b.display().to_lowercase()));
    out
}

pub fn import_handler() -> Result<(), Box<dyn Error>> {
    let Some(file) = FileDialog::new()
        .set_title("Select File")
        .set_directory(&*PATH_HOME)
        .add_filter("Splitux Handler Package", &["spx"])
        .pick_file()
    else {
        return Ok(());
    };

    if !file.exists() || !file.is_file() || file.extension().unwrap_or_default() != "spx" {
        return Err("Handler not valid!".into());
    }

    let dir_handlers = PATH_PARTY.join("handlers");
    let dir_tmp = PATH_PARTY.join("tmp");
    if !dir_tmp.exists() {
        std::fs::create_dir_all(&dir_tmp)?;
    }

    let mut archive = zip::ZipArchive::new(File::open(&file)?)?;
    archive.extract(&dir_tmp)?;

    let handler_path = dir_tmp.join("handler.yaml");
    if !handler_path.exists() {
        clear_tmp()?;
        return Err("handler.yaml not found in archive".into());
    }

    let mut fileclone = file.clone();
    fileclone.set_extension("");
    let name = fileclone
        .file_name()
        .ok_or_else(|| "No filename")?
        .to_string_lossy();

    let path = {
        if !dir_handlers.join(name.as_ref()).exists() {
            dir_handlers.join(name.as_ref())
        } else {
            let mut i = 1;
            while PATH_PARTY
                .join("handlers")
                .join(&format!("{}-{}", name, i))
                .exists()
            {
                i += 1;
            }
            dir_handlers.join(&format!("{}-{}", name, i))
        }
    };

    copy_dir_recursive(&dir_tmp, &path)?;
    clear_tmp()?;

    Ok(())
}
