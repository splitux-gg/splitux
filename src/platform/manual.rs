//! Manual path platform (no store)

use super::Platform;
use std::error::Error;
use std::path::PathBuf;

pub struct ManualPlatform {
    pub path: String,
}

impl ManualPlatform {
    pub fn new(path: String) -> Self {
        Self { path }
    }
}

impl Platform for ManualPlatform {
    fn name(&self) -> &str {
        "manual"
    }

    fn game_root_path(&self) -> Result<PathBuf, Box<dyn Error>> {
        Ok(PathBuf::from(&self.path))
    }
}
