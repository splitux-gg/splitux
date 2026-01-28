use std::cell::RefCell;

/// Get monitors directly from niri IPC (bypasses SDL entirely).
///
/// Returns None if not running on niri or if query fails.
/// Only returns enabled outputs (those with a "logical" field).
fn get_monitors_niri() -> Option<Vec<Monitor>> {
    // Only run on niri
    if std::env::var("NIRI_SOCKET").is_err() {
        return None;
    }

    let output = std::process::Command::new("niri")
        .args(["msg", "--json", "outputs"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let obj = json.as_object()?;

    let mut monitors = Vec::new();
    for (connector, info) in obj {
        // Enabled outputs have "logical" with width/height
        if let Some(logical) = info.get("logical") {
            if logical.is_null() {
                continue;
            }
            let width = logical.get("width")?.as_u64()? as u32;
            let height = logical.get("height")?.as_u64()? as u32;
            monitors.push(Monitor {
                name: connector.clone(),
                width,
                height,
            });
        }
    }

    // Sort by logical x position for consistent ordering
    monitors.sort_by_key(|m| {
        if let Some(info) = obj.get(&m.name) {
            if let Some(logical) = info.get("logical") {
                return logical.get("x").and_then(|x| x.as_i64()).unwrap_or(0);
            }
        }
        0
    });

    Some(monitors)
}

// Cached SDL video subsystem to avoid repeated initialization.
//
// Re-initializing SDL2 on every monitor poll creates new XWayland connections
// each time, which can interfere with the eframe/winit event loop and cause
// UI unresponsiveness on Wayland compositors like niri.
//
// We use thread_local because SDL2's VideoSubsystem is not thread-safe (doesn't
// implement Send/Sync), but all monitor polling happens on the main UI thread.
thread_local! {
    static SDL_VIDEO: RefCell<Option<sdl2::VideoSubsystem>> = const { RefCell::new(None) };
}

fn with_sdl_video<T>(f: impl FnOnce(&sdl2::VideoSubsystem) -> T) -> T {
    SDL_VIDEO.with(|cell| {
        let mut opt = cell.borrow_mut();
        if opt.is_none() {
            *opt = Some(
                sdl2::init()
                    .expect("Failed to initialize SDL2")
                    .video()
                    .expect("Failed to initialize SDL2 video"),
            );
        }
        f(opt.as_ref().unwrap())
    })
}

#[derive(Clone)]
pub struct Monitor {
    name: String,
    width: u32,
    height: u32,
}

impl Monitor {
    /// Full display name as reported by SDL (e.g., "HDMI-A-1 24\"")
    pub fn name(&self) -> &str {
        &self.name
    }

    /// User-friendly display name for UI.
    ///
    /// Just returns the connector name (DP-1, HDMI-A-1, etc.) since that's
    /// the most reliable and consistent identifier across backends.
    pub fn display_name(&self) -> String {
        self.connector_name().to_string()
    }

    /// DRM connector name without size suffix (e.g., "HDMI-A-1")
    ///
    /// SDL formats monitor names differently depending on the backend:
    /// - X11: "HDMI-A-1 24\"" (connector first, then size)
    /// - Wayland: "PNP(XEC) - ES-G24F4L - HDMI-A-1" (manufacturer - model - connector)
    ///
    /// This extracts just the connector portion which matches what window
    /// managers like niri use.
    pub fn connector_name(&self) -> &str {
        // DRM connector patterns: DP-N, HDMI-A-N, VGA-N, DVI-D-N, eDP-N, etc.
        let is_connector = |s: &str| {
            let prefixes = ["DP-", "HDMI-", "VGA-", "DVI-", "eDP-", "LVDS-", "DSI-"];
            prefixes.iter().any(|p| s.starts_with(p))
        };

        // Try Wayland format: last segment after " - " that looks like a connector
        if self.name.contains(" - ") {
            if let Some(connector) = self.name.rsplit(" - ").find(|s| is_connector(s)) {
                return connector;
            }
        }

        // Try X11 format: first token before space
        self.name.split_whitespace().next().unwrap_or(&self.name)
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }
}

/// Get monitor information.
///
/// On niri: Uses niri IPC directly (no SDL, no XWayland).
/// On other compositors: Uses SDL2 with X11 backend.
pub fn get_monitors_sdl() -> Vec<Monitor> {
    // On niri, bypass SDL entirely to avoid XWayland issues
    if let Some(monitors) = get_monitors_niri() {
        return monitors;
    }

    // Fall back to SDL for other compositors
    with_sdl_video(|video| {
        let count = video.num_video_displays().unwrap_or(0);
        let mut monitors = Vec::new();
        for i in 0..count {
            if let (Ok(name), Ok(bounds)) = (video.display_name(i), video.display_bounds(i)) {
                // Filter out XWAYLAND virtual displays
                let connector = name.split_whitespace().next().unwrap_or(&name);
                if connector.starts_with("XWAYLAND") {
                    continue;
                }

                monitors.push(Monitor {
                    name,
                    width: bounds.width(),
                    height: bounds.height(),
                });
            }
        }
        monitors
    })
}
