use crate::Monitor;
use crate::app::SplituxConfig;
use crate::profiles::GUEST_NAMES;
use crate::wm::types::{get_layout_type, LayoutType};

/// Whether the layout selected for this player count gives every instance a
/// full-monitor-resolution surface (vs splitting the monitor between them).
fn is_fullscreen_layout(cfg: &SplituxConfig, player_count: usize) -> bool {
    get_layout_type(cfg.layout_presets.get_for_count(player_count)) == LayoutType::Fullscreen
}

/// How a remote (Together) player's input is presented to the game. The seat
/// always exposes a virtual pad + kbd + mouse and the kbd/mouse are always held
/// by gamescope (so remote keystrokes can't leak to the host desktop); this only
/// controls whether the game ALSO sees this player as a gamepad.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TogetherInput {
    /// Wire the seat's virtual pad into the game's SDL (couch-co-op default).
    #[default]
    Gamepad,
    /// No gamepad identity for this player — they drive the held kbd/mouse only,
    /// so a pad-based game doesn't invent a spurious extra player.
    KbMouse,
}

impl TogetherInput {
    pub fn label(self) -> &'static str {
        match self {
            TogetherInput::Gamepad => "Gamepad",
            TogetherInput::KbMouse => "Kb+Mouse",
        }
    }
    /// Toggle between the two (for the card's cycle control).
    pub fn next(self) -> Self {
        match self {
            TogetherInput::Gamepad => TogetherInput::KbMouse,
            TogetherInput::KbMouse => TogetherInput::Gamepad,
        }
    }
}

#[derive(Clone)]
pub struct Instance {
    pub devices: Vec<usize>,
    pub profname: String,
    pub profselection: usize,
    pub monitor: usize,
    pub width: u32,
    pub height: u32,
    /// When true, this player is a remote Together seat: a seat-streamer owns
    /// its input + streams its screen to a browser. See [`crate::together`].
    pub together: bool,
    /// How this remote player's input reaches the game (ignored unless
    /// `together`).
    pub together_input: TogetherInput,
    /// Number of remote Together seats this instance owns. Normally 1 when
    /// `together` (online/LAN: one instance ↔ one seat). For a local-split
    /// (couch-co-op) game the players collapse into a single instance that owns
    /// N seats — N browsers driving the one shared game. 0 when not `together`.
    pub together_seats: u8,
    /// True when a LOCAL (non-Together) player drives this instance — i.e. the
    /// host sits at this shared game with their own kb/m or pad. Only meaningful
    /// for a collapsed local-split instance that ALSO owns remote seats: gamescope
    /// holds the seats' virtual devices, which otherwise blocks ALL parent
    /// compositor input, so the host's focus-driven kb/m would be locked out.
    /// When set, splitux passes gamescope `--libinput-allow-parent` so the host's
    /// input still reaches the game alongside the held remote devices. See
    /// [`crate::together::collapse_for_local_split`].
    pub local_input: bool,
}

pub fn set_instance_resolutions(
    instances: &mut Vec<Instance>,
    primary_monitor: &Monitor,
    cfg: &SplituxConfig,
) {
    let (basewidth, baseheight) = (primary_monitor.width(), primary_monitor.height());
    let playercount = instances.len();
    let fullscreen = is_fullscreen_layout(cfg, playercount);

    for instance in instances {
        // Fullscreen layout: every instance renders at full monitor resolution.
        let (mut w, mut h) = if fullscreen {
            (basewidth, baseheight)
        } else {
            match playercount {
                1 => (basewidth, baseheight),
                2 => {
                    // Check layout_presets for vertical vs horizontal
                    let is_vertical = cfg.layout_presets.two_player.contains("vertical");
                    if is_vertical {
                        (basewidth / 2, baseheight)
                    } else {
                        (basewidth, baseheight / 2)
                    }
                }
                _ => (basewidth / 2, baseheight / 2),
            }
        };
        if h < 600 && cfg.gamescope_fix_lowres {
            let ratio = w as f32 / h as f32;
            h = 600;
            w = (h as f32 * ratio) as u32;
        }
        instance.width = w;
        instance.height = h;
    }
}

pub fn set_instance_resolutions_multimonitor(
    instances: &mut Vec<Instance>,
    monitors: &Vec<Monitor>,
    cfg: &SplituxConfig,
) {
    // The fullscreen preset is keyed by the TOTAL player count (matching the
    // launch UI), and gives every instance a full surface on its own monitor.
    let fullscreen = is_fullscreen_layout(cfg, instances.len());

    let mut mon_playercounts: Vec<usize> = vec![0; monitors.len()];
    for instance in instances.iter() {
        let mon = instance.monitor;
        mon_playercounts[mon] += 1;
    }

    for instance in instances.iter_mut() {
        let playercount = mon_playercounts[instance.monitor];
        let (basewidth, baseheight) = (
            monitors[instance.monitor].width(),
            monitors[instance.monitor].height(),
        );

        let (mut w, mut h) = if fullscreen {
            (basewidth, baseheight)
        } else {
            match playercount {
                1 => (basewidth, baseheight),
                2 => {
                    // Check layout_presets for vertical vs horizontal
                    let is_vertical = cfg.layout_presets.two_player.contains("vertical");
                    if is_vertical {
                        (basewidth / 2, baseheight)
                    } else {
                        (basewidth, baseheight / 2)
                    }
                }
                _ => (basewidth / 2, baseheight / 2),
            }
        };
        if h < 600 && cfg.gamescope_fix_lowres {
            let ratio = w as f32 / h as f32;
            h = 600;
            w = (h as f32 * ratio) as u32;
        }
        instance.width = w;
        instance.height = h;
    }
}

pub fn set_instance_names(instances: &mut Vec<Instance>, profiles: &[String]) {
    let mut guests = GUEST_NAMES.to_vec();

    for instance in instances {
        if instance.profselection == 0 {
            let i = fastrand::usize(..guests.len());
            instance.profname = format!(".{}", guests[i]);
            guests.swap_remove(i);
        } else {
            instance.profname = profiles[instance.profselection].to_owned();
        }
    }
}
