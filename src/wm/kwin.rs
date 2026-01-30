//! KWin window manager integration via D-Bus scripting API

use crate::paths::PATH_ASSETS;
use crate::wm::{LayoutContext, NestedSession, WindowManager, WmResult};
use crate::monitor::Monitor;
use std::path::PathBuf;
use std::process::Command;

pub struct KWinManager {
    script_loaded: bool,
}

impl KWinManager {
    pub fn new() -> Self {
        Self {
            script_loaded: false,
        }
    }

    fn load_script(&mut self, file: PathBuf) -> WmResult<()> {
        println!(
            "[splitux] wm::kwin - Loading script {}...",
            file.display()
        );

        if !file.exists() {
            return Err(format!(
                "KWin script file doesn't exist: {}",
                file.display()
            )
            .into());
        }

        let conn = zbus::blocking::Connection::session()?;
        let proxy = zbus::blocking::Proxy::new(
            &conn,
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting",
        )?;

        let _: i32 = proxy.call("loadScript", &(file.to_string_lossy(), "splitscreen"))?;
        println!("[splitux] wm::kwin - Script loaded. Starting...");
        let _: () = proxy.call("start", &())?;

        self.script_loaded = true;
        println!("[splitux] wm::kwin - KWin script started.");
        Ok(())
    }

    fn unload_script(&mut self) -> WmResult<()> {
        if !self.script_loaded {
            return Ok(());
        }

        println!("[splitux] wm::kwin - Unloading splitscreen script...");
        let conn = zbus::blocking::Connection::session()?;
        let proxy = zbus::blocking::Proxy::new(
            &conn,
            "org.kde.KWin",
            "/Scripting",
            "org.kde.kwin.Scripting",
        )?;

        let _: bool = proxy.call("unloadScript", &("splitscreen",))?;
        self.script_loaded = false;

        println!("[splitux] wm::kwin - Script unloaded.");
        Ok(())
    }
}

impl Default for KWinManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowManager for KWinManager {
    fn name(&self) -> &'static str {
        "KWin"
    }

    fn setup(&mut self, ctx: &LayoutContext) -> WmResult<()> {
        // For 2-player, check if using vertical preset; otherwise use horizontal script
        // TODO: Generate dynamic KWin scripts from preset coordinates
        let script = if ctx.preset.id == "2p_vertical" {
            "splitscreen_kwin_vertical.js"
        } else {
            "splitscreen_kwin.js"
        };
        self.load_script(PATH_ASSETS.join(script))
    }

    fn teardown(&mut self) -> WmResult<()> {
        self.unload_script()
    }

    fn is_available() -> bool {
        // Check if KWin D-Bus service is available
        if let Ok(conn) = zbus::blocking::Connection::session() {
            let proxy = zbus::blocking::Proxy::new(
                &conn,
                "org.kde.KWin",
                "/Scripting",
                "org.kde.kwin.Scripting",
            );
            return proxy.is_ok();
        }
        false
    }

    fn is_reactive(&self) -> bool {
        // KWin uses a script that reacts to window events
        true
    }
}

impl NestedSession for KWinManager {
    fn nested_session_command(
        &self,
        splitux_args: &[String],
        monitor: &Monitor,
    ) -> Command {
        let mut cmd = Command::new("kwin_wayland");
        cmd.arg("--xwayland");
        cmd.arg("--width").arg(monitor.width().to_string());
        cmd.arg("--height").arg(monitor.height().to_string());
        cmd.arg("--exit-with-session");

        let args_string = splitux_args
            .iter()
            .map(|arg| format!("\"{}\"", arg))
            .collect::<Vec<String>>()
            .join(" ");
        cmd.arg(args_string);

        cmd
    }
}
