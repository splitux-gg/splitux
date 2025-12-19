// Steam asset discovery and image resolution for handlers
//
// Handler artwork methods delegate to the Platform trait for platform-specific
// asset resolution (Steam cache, etc.), with local handler directory overrides.

use crate::handler::Handler;
use eframe::egui::{self, ImageSource};
use std::path::PathBuf;

impl Handler {
    /// Get icon for this handler
    ///
    /// Resolution order:
    /// 1. Local icon in handler directory (icon.png, icon.jpg, icon.ico)
    /// 2. Platform-provided icon (e.g., Steam cache)
    /// 3. Default executable icon
    pub fn icon(&self) -> ImageSource<'_> {
        // Check for local icon first (supports .png, .jpg, .ico)
        let local_icon_png = self.path_handler.join("icon.png");
        let local_icon_jpg = self.path_handler.join("icon.jpg");
        let local_icon_ico = self.path_handler.join("icon.ico");
        if local_icon_png.exists() {
            return format!("file://{}", local_icon_png.display()).into();
        }
        if local_icon_jpg.exists() {
            return format!("file://{}", local_icon_jpg.display()).into();
        }
        if local_icon_ico.exists() {
            return format!("file://{}", local_icon_ico.display()).into();
        }

        // Delegate to Platform trait for platform-specific icon
        if let Some(uri) = self.get_platform().icon_uri() {
            return uri.into();
        }

        // Fallback to default icon
        egui::include_image!("../../../res/executable_icon.png")
    }

    /// Returns the box art from platform (e.g., Steam's library_600x900.jpg)
    pub fn box_art(&self) -> Option<String> {
        self.get_platform().box_art_uri()
    }

    /// Returns the game logo from platform (e.g., Steam's logo.png)
    pub fn logo_image(&self) -> Option<String> {
        self.get_platform().logo_uri()
    }

    /// Returns the hero image from platform (e.g., Steam's library_hero.jpg 1920x620 banner)
    pub fn hero_image(&self) -> Option<String> {
        self.get_platform().hero_uri()
    }

    /// Get local images from handler's imgs directory
    pub(crate) fn get_imgs(&self) -> Vec<PathBuf> {
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
}
