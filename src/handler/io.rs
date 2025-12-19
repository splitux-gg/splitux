//! Handler I/O operations
//!
//! Standalone I/O functions for handler scanning and import.
//! Handler methods (from_yaml, save, export) remain on Handler struct in handler_legacy.rs
//! until full migration is complete.

use crate::paths::{PATH_HOME, PATH_PARTY};
use crate::util::{clear_tmp, copy_dir_recursive};

use rfd::FileDialog;
use std::error::Error;
use std::fs::File;

use super::Handler;

/// Scan the handlers directory and load all valid handlers
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

/// Import a handler from a .spx package file
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
