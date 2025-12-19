// Handler persistence operations - save, export, remove

use crate::handler::Handler;
use crate::paths::{PATH_HOME, PATH_PARTY};
use crate::util::{clear_tmp, copy_dir_recursive, zip_dir};
use rfd::FileDialog;
use std::error::Error;

impl Handler {
    pub fn remove_handler(&self) -> Result<(), Box<dyn Error>> {
        if !self.is_saved_handler() {
            return Err("No handler directory to remove".into());
        }
        std::fs::remove_dir_all(self.path_handler.clone())?;
        Ok(())
    }

    pub fn get_game_rootpath(&self) -> Result<String, Box<dyn Error>> {
        // Use Platform trait for unified path resolution
        let platform = self.get_platform();
        let path = platform.game_root_path()?;
        Ok(path.to_string_lossy().to_string())
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
