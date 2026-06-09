use crate::Monitor;
use crate::app::SplituxConfig;
use crate::profiles::GUEST_NAMES;
use crate::wm::types::{get_layout_type, LayoutType};

/// Whether the layout selected for this player count gives every instance a
/// full-monitor-resolution surface (vs splitting the monitor between them).
fn is_fullscreen_layout(cfg: &SplituxConfig, player_count: usize) -> bool {
    get_layout_type(cfg.layout_presets.get_for_count(player_count)) == LayoutType::Fullscreen
}

#[derive(Clone)]
pub struct Instance {
    pub devices: Vec<usize>,
    pub profname: String,
    pub profselection: usize,
    pub monitor: usize,
    pub width: u32,
    pub height: u32,
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
