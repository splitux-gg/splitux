// Window polling loop — shared between niri and hyprland

use crate::wm::WmResult;

/// Poll for windows until we find the expected count, or timeout.
///
/// `wm_name`: name for logging (e.g., "niri", "hyprland")
/// `expected`: number of windows to wait for
/// `get_count`: closure that returns the current window count (performs I/O)
pub fn wait_for_windows<F>(wm_name: &str, expected: usize, get_count: F) -> WmResult<usize>
where
    F: Fn() -> usize,
{
    let max_wait = std::time::Duration::from_secs(120);
    let poll_interval = std::time::Duration::from_millis(500);
    let start = std::time::Instant::now();

    loop {
        let count = get_count();

        if count >= expected {
            println!(
                "[splitux] wm::{} - Found {} windows after {:.1}s",
                wm_name,
                count,
                start.elapsed().as_secs_f32()
            );
            std::thread::sleep(std::time::Duration::from_millis(500));
            return Ok(count);
        }

        if start.elapsed() > max_wait {
            println!(
                "[splitux] wm::{} - Timeout waiting for windows ({}/{})",
                wm_name, count, expected
            );
            return Ok(count);
        }

        if start.elapsed().as_secs() % 5 == 0 && start.elapsed().as_millis() % 500 < 100 {
            println!(
                "[splitux] wm::{} - Still waiting... ({}/{} windows)",
                wm_name, count, expected
            );
        }

        std::thread::sleep(poll_interval);
    }
}
