mod app;
mod backend_legacy;
mod backend;
mod bwrap;
mod facepunch;
mod game_patches;
mod gamescope;
mod goldberg;
mod handler;
mod input;
mod instance;
mod launch_legacy;
mod launch;
mod monitor;
mod paths;
mod photon;
mod platform;
mod profiles;
mod proton;
mod registry;
mod save_sync;
mod util;
mod wm;

use crate::app::*;
use crate::handler::Handler;
use crate::monitor::*;
use crate::paths::PATH_PARTY;
use crate::profiles::remove_guest_profiles;
use crate::util::*;

fn main() -> eframe::Result {
    // Our sdl/multimonitor stuff essentially depends on us running through x11.
    unsafe {
        std::env::set_var("SDL_VIDEODRIVER", "x11");
    }

    let monitors = get_monitors_sdl();

    println!("[splitux] Monitors detected:");
    for monitor in &monitors {
        println!(
            "[splitux] {} ({}x{})",
            monitor.name(),
            monitor.width(),
            monitor.height()
        );
    }

    let args: Vec<String> = std::env::args().collect();

    if std::env::args().any(|arg| arg == "--help") {
        println!("{}", USAGE_TEXT);
        std::process::exit(0);
    }

    if std::env::args().any(|arg| arg == "--kwin") {
        use crate::wm::{KWinManager, NestedSession};

        let args: Vec<String> = std::env::args().filter(|arg| arg != "--kwin").collect();
        let kwin = KWinManager::new();
        let mut cmd = kwin.nested_session_command(&args, &monitors[0]);

        println!("[splitux] Launching kwin session: {:?}", cmd);

        match cmd.spawn() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                eprintln!("[splitux] Failed to start kwin_wayland: {}", e);
                std::process::exit(1);
            }
        }
    }

    if std::env::args().any(|arg| arg == "--hyprland") {
        use crate::wm::{HyprlandManager, NestedSession};

        let args: Vec<String> = std::env::args().filter(|arg| arg != "--hyprland").collect();
        let hyprland = HyprlandManager::new();
        let mut cmd = hyprland.nested_session_command(&args, &monitors[0]);

        println!("[splitux] Launching hyprland session: {:?}", cmd);

        match cmd.spawn() {
            Ok(_) => std::process::exit(0),
            Err(e) => {
                eprintln!("[splitux] Failed to start Hyprland: {}", e);
                std::process::exit(1);
            }
        }
    }

    let mut exec = String::new();
    let mut execargs = String::new();
    if let Some(exec_index) = args.iter().position(|arg| arg == "--exec") {
        if let Some(next_arg) = args.get(exec_index + 1) {
            exec = next_arg.clone();
        } else {
            eprintln!("{}", USAGE_TEXT);
            std::process::exit(1);
        }
    }
    if let Some(execargs_index) = args.iter().position(|arg| arg == "--args") {
        if let Some(next_arg) = args.get(execargs_index + 1) {
            execargs = next_arg.clone();
        } else {
            eprintln!("{}", USAGE_TEXT);
            std::process::exit(1);
        }
    }

    let handler_lite = if !exec.is_empty() {
        Some(Handler::from_cli(&exec, &execargs))
    } else {
        None
    };

    let fullscreen = std::env::args().any(|arg| arg == "--fullscreen");

    std::fs::create_dir_all(PATH_PARTY.join("handlers"))
        .expect("Failed to create handlers directory");
    std::fs::create_dir_all(PATH_PARTY.join("profiles"))
        .expect("Failed to create profiles directory");
    if !PATH_PARTY.join("goldberg_data").exists() {
        std::fs::create_dir_all(PATH_PARTY.join("goldberg_data/steam_settings"))
            .expect("Failed to create goldberg data!");
        std::fs::write(PATH_PARTY.join("goldberg_data/steam_settings/auto_accept_invite.txt"), "").expect("failed to create auto_accept_invite.txt");
        std::fs::write(PATH_PARTY.join("goldberg_data/steam_settings/auto_send_invite.txt"), "").expect("failed to create auto_send_invite.txt");
    }

    remove_guest_profiles().unwrap();
    clear_tmp().unwrap();

    let scrheight = monitors[0].height();

    let scale = match fullscreen {
        true => scrheight as f32 / 560.0,
        false => 1.3,
    };

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 540.0])
            .with_min_inner_size([640.0, 360.0])
            .with_fullscreen(fullscreen)
            .with_icon(
                eframe::icon_data::from_png_bytes(&include_bytes!("../res/icon.png")[..])
                    .expect("Failed to load icon"),
            ),
        ..Default::default()
    };

    println!("[splitux] Starting eframe app...");

    eframe::run_native(
        "Splitux",
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            cc.egui_ctx.set_zoom_factor(scale);

            // Apply custom theme
            crate::app::theme::apply_theme(&cc.egui_ctx);

            Ok(Box::<PartyApp>::new(PartyApp::new(
                monitors.clone(),
                handler_lite,
            )))
        }),
    )
}

static USAGE_TEXT: &str = r#"
{}
Usage: splitux [OPTIONS]

Options:
    --exec <executable>   Execute the specified executable in splitscreen. If this isn't specified, Splitux will launch in the regular GUI mode.
    --args [args]         Specify arguments for the executable to be launched with. Must be quoted if containing spaces.
    --fullscreen          Start the GUI in fullscreen mode
    --kwin                Launch Splitux inside of a nested KWin session
    --hyprland            Launch Splitux inside of a nested Hyprland session
"#;
