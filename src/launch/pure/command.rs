// Pure command building functions (no I/O)

use std::path::Path;
use std::process::Command;

/// Reconstruct a Command with extra args inserted at a specific position.
///
/// This is used to insert device-blocking bwrap args (--bind /dev/null /dev/input/...)
/// between the bwrap options and the child command (proton/game). The insertion
/// happens at spawn time so device permissions are checked against current state,
/// not stale build-time state.
pub fn rebuild_command_with_blocking(
    cmd: Command,
    insertion_idx: usize,
    extra_args: &[String],
) -> Command {
    if extra_args.is_empty() {
        return cmd;
    }

    let program = cmd.get_program().to_owned();
    let all_args: Vec<std::ffi::OsString> = cmd.get_args().map(|a| a.to_owned()).collect();
    let envs: Vec<_> = cmd
        .get_envs()
        .map(|(k, v)| (k.to_owned(), v.map(|v| v.to_owned())))
        .collect();
    let cwd = cmd.get_current_dir().map(|p| p.to_owned());

    let mut new_cmd = Command::new(&program);

    // Args before insertion point (gamescope + bwrap options)
    new_cmd.args(&all_args[..insertion_idx]);

    // Device blocking args
    for arg in extra_args {
        new_cmd.arg(arg);
    }

    // Args after insertion point (runtime + game exe + handler args)
    new_cmd.args(&all_args[insertion_idx..]);

    // Restore environment variables
    for (key, val) in &envs {
        match val {
            Some(v) => {
                new_cmd.env(key, v);
            }
            None => {
                new_cmd.env_remove(key);
            }
        }
    }

    // Restore working directory
    if let Some(dir) = &cwd {
        new_cmd.current_dir(dir);
    }

    new_cmd
}

/// Format a launch command for debug logging (pure string building).
///
/// Returns the formatted string. Caller is responsible for printing.
pub fn format_launch_cmd(cmd: &Command, i: usize) -> String {
    let mut output = String::new();

    output.push_str(&format!("[splitux] INSTANCE {}:\n", i + 1));

    let cwd = cmd.get_current_dir().unwrap_or_else(|| Path::new(""));
    output.push_str(&format!("[splitux] CWD={}\n", cwd.display()));

    for var in cmd.get_envs() {
        let value = var.1.ok_or_else(|| "").unwrap_or_default();
        output.push_str(&format!(
            "[splitux] {}={}\n",
            var.0.to_string_lossy(),
            value.display()
        ));
    }

    output.push_str(&format!("[splitux] \"{}\"\n", cmd.get_program().display()));

    output.push_str("[splitux] ");
    for arg in cmd.get_args() {
        let fmtarg = arg.to_string_lossy();
        if fmtarg == "--bind"
            || fmtarg == "bwrap"
            || (fmtarg.starts_with("/") && fmtarg.len() > 1)
        {
            output.push_str("\n[splitux] ");
        } else {
            output.push(' ');
        }
        output.push_str(&format!("\"{}\"", fmtarg));
    }

    output.push_str("\n[splitux] ---------------------");
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    // ── rebuild_command_with_blocking ──

    #[test]
    fn rebuild_empty_extra_args_returns_original() {
        let cmd = Command::new("bwrap");
        let result = rebuild_command_with_blocking(cmd, 0, &[]);
        assert_eq!(result.get_program(), "bwrap");
        assert_eq!(result.get_args().count(), 0);
    }

    #[test]
    fn rebuild_insertion_at_index_zero() {
        let mut cmd = Command::new("bwrap");
        cmd.args(["--dev-bind", "/", "/", "game.exe"]);

        let extra = vec![
            "--bind".to_string(),
            "/dev/null".to_string(),
            "/dev/input/event0".to_string(),
        ];

        let result = rebuild_command_with_blocking(cmd, 0, &extra);

        assert_eq!(result.get_program(), "bwrap");
        let args: Vec<String> = result
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "--bind",
                "/dev/null",
                "/dev/input/event0",
                "--dev-bind",
                "/",
                "/",
                "game.exe",
            ]
        );
    }

    #[test]
    fn rebuild_insertion_at_middle_index() {
        let mut cmd = Command::new("bwrap");
        cmd.args(["--dev-bind", "/", "/", "game.exe"]);

        let extra = vec![
            "--bind".to_string(),
            "/dev/null".to_string(),
            "/dev/input/event0".to_string(),
        ];

        let result = rebuild_command_with_blocking(cmd, 2, &extra);

        let args: Vec<String> = result
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            vec![
                "--dev-bind",
                "/",
                "--bind",
                "/dev/null",
                "/dev/input/event0",
                "/",
                "game.exe",
            ]
        );
    }

    #[test]
    fn rebuild_preserves_env_vars() {
        let mut cmd = Command::new("bwrap");
        cmd.args(["--dev-bind", "/", "/"]);
        cmd.env("FOO", "bar");

        let extra = vec!["--extra".to_string()];
        let result = rebuild_command_with_blocking(cmd, 0, &extra);

        let envs: Vec<_> = result
            .get_envs()
            .filter(|(k, _)| *k == "FOO")
            .collect();
        assert_eq!(envs.len(), 1);
        assert_eq!(envs[0].1, Some(std::ffi::OsStr::new("bar")));
    }

    #[test]
    fn rebuild_preserves_current_dir() {
        let mut cmd = Command::new("bwrap");
        cmd.args(["--dev-bind", "/", "/"]);
        cmd.current_dir("/tmp");

        let extra = vec!["--extra".to_string()];
        let result = rebuild_command_with_blocking(cmd, 0, &extra);

        assert_eq!(
            result.get_current_dir(),
            Some(Path::new("/tmp"))
        );
    }

    // ── format_launch_cmd ──

    #[test]
    fn format_instance_numbering_is_one_based() {
        let cmd = Command::new("game");
        let output = format_launch_cmd(&cmd, 0);
        assert!(output.contains("INSTANCE 1:"));
    }

    #[test]
    fn format_contains_program_name_in_quotes() {
        let cmd = Command::new("my_game");
        let output = format_launch_cmd(&cmd, 0);
        assert!(output.contains("\"my_game\""));
    }

    #[test]
    fn format_contains_cwd_when_set() {
        let mut cmd = Command::new("game");
        cmd.current_dir("/home/user/games");
        let output = format_launch_cmd(&cmd, 0);
        assert!(output.contains("CWD=/home/user/games"));
    }

    #[test]
    fn format_contains_env_vars_when_set() {
        let mut cmd = Command::new("game");
        cmd.env("STEAM_COMPAT_DATA_PATH", "/pfx");
        let output = format_launch_cmd(&cmd, 0);
        assert!(output.contains("STEAM_COMPAT_DATA_PATH=/pfx"));
    }

    #[test]
    fn format_contains_separator_at_end() {
        let cmd = Command::new("game");
        let output = format_launch_cmd(&cmd, 0);
        assert!(output.ends_with("[splitux] ---------------------"));
    }
}
