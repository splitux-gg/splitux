//! Keen (Keen Games online backend) emulator backend.
//!
//! For titles gated on Keen's proprietary online backend — e.g. Enshrouded,
//! whose multiplayer is locked behind an auth server at
//! `eonlinedb.enshrouded.com:27503` (NOT Steam). The game refuses to host/join
//! until it logs into that server, so Goldberg alone can't bridge it.
//!
//! Unlike Goldberg/EOS (in-process DLL replacements), Keen is a real TCP auth
//! server, so this backend is a **shared sidecar**: it runs the bundled
//! `keen-emu` binary on loopback, writes the server "data file" (with our
//! substituted pinned X25519/Ed25519 keys), and injects
//! `--keenonline-server-data-file <file>` into the game so it authenticates
//! against the emu instead of real Keen.
//!
//! Co-exists with Goldberg: Keen is the *auth gate* (flips the game "online" so
//! it will create/join a Steam lobby), Goldberg carries the actual lobby
//! discovery + P2P data. Enable both in the handler.

use super::Backend;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use crate::handler::Handler;
use crate::instance::Instance;
use crate::paths::BIN_KEEN_EMU;

/// Keen emulator settings from handler YAML (dot-notation: keen.*)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct KeenSettings {
    /// Loopback listen address for the emu (keen.addr). Must match the auth
    /// server address the game connects to; the emu writes it into the data file
    /// as `addressAndPort`, so it can be any free loopback port.
    #[serde(default = "default_addr")]
    pub addr: String,
}

fn default_addr() -> String {
    "127.0.0.1:27503".to_string()
}

impl Default for KeenSettings {
    fn default() -> Self {
        Self { addr: default_addr() }
    }
}

/// Keen backend implementation
pub struct Keen {
    pub settings: KeenSettings,
}

impl Keen {
    pub fn new(settings: KeenSettings) -> Self {
        Self { settings }
    }
}

/// Path the emu writes its data file to (shared across all instances of the run).
///
/// Per-launch namespaced (`tmp/<launch_ns>/keen/`) so a SECOND concurrent splitux
/// process can't overwrite a running session's freshly-generated keypair file
/// mid-game. NB: the emu's loopback PORT (`settings.addr`, default 27503) is
/// still fixed, so two concurrent Keen-gated sessions on one host still serialize
/// on that port — a per-launch port would need the keen-emu binary to advertise
/// its bound port back into this data file.
fn data_file_path() -> PathBuf {
    crate::paths::launch_tmp_dir().join("keen").join("keenonline-emu.json")
}

impl Backend for Keen {
    fn name(&self) -> &str {
        "keen"
    }

    /// No per-instance filesystem overlay: the emu is a shared sidecar and the
    /// only per-launch artifact is a single data file referenced by a CLI arg.
    fn requires_overlay(&self) -> bool {
        false
    }

    fn create_all_overlays(
        &self,
        _handler: &Handler,
        instances: &[Instance],
        _global_indices: &[usize],
        _is_windows: bool,
        _game_root: &Path,
    ) -> Result<Vec<PathBuf>, Box<dyn Error>> {
        // Not used (requires_overlay() == false); return empty paths parallel to
        // instances for callers that map by index.
        Ok(instances.iter().map(|_| PathBuf::new()).collect())
    }

    /// Inject `--keenonline-server-data-file <file>` so the game points its Keen
    /// auth at the emu. For Proton/wine titles the path is given as a Windows
    /// path on the `Z:` drive (which wine maps to `/`).
    fn extra_launch_args(&self, _handler: &Handler, is_windows: bool) -> Vec<String> {
        // Lockstep with start_services' presence guard: if the sideloaded emu
        // isn't installed we never start it, so don't point the game at a data
        // file it never wrote (that just hangs/fails the Keen handshake). Inject
        // nothing and let the game run without the Keen auth gate.
        if !BIN_KEEN_EMU.exists() {
            return Vec::new();
        }
        let p = data_file_path();
        let path_arg = if is_windows {
            format!("Z:{}", p.to_string_lossy().replace('/', "\\"))
        } else {
            p.to_string_lossy().to_string()
        };
        vec!["--keenonline-server-data-file".to_string(), path_arg]
    }

    /// Start the shared keen-emu auth server. It generates fresh keypairs, writes
    /// the data file (which `extra_launch_args` points the game at), and listens
    /// on `settings.addr`. Returns the child so the session can kill it at
    /// teardown.
    fn start_services(&self, _handler: &Handler) -> std::io::Result<Vec<Child>> {
        let bin = BIN_KEEN_EMU.clone();
        // Sideloaded backend asset (splitux-gg/keen-emu-splitux): if it isn't
        // installed, degrade cleanly with a clear log instead of spawn-erroring
        // and leaving the game pointed at a data file the absent emu never wrote
        // (which would silently fail the auth handshake). Mirrors the
        // seat-streamer presence guard.
        if !bin.exists() {
            println!(
                "[splitux] keen - keen-emu not found at {} — Keen-gated multiplayer disabled \
                 (install splitux-gg/keen-emu-splitux via `splitux.sh build`)",
                bin.display()
            );
            return Ok(Vec::new());
        }
        let data_file = data_file_path();
        if let Some(parent) = data_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let child = Command::new(&bin)
            .env("KEEN_ADDR", &self.settings.addr)
            .env("KEEN_DATA_FILE", &data_file)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?;

        // Give the emu a moment to bind and write the data file before any game
        // instance launches and tries to connect / read the file.
        std::thread::sleep(std::time::Duration::from_millis(500));
        println!(
            "[splitux] keen-emu started (pid {}) on {} -> {}",
            child.id(),
            self.settings.addr,
            data_file.display()
        );
        Ok(vec![child])
    }
}
