//! Helper methods for Splitux

use super::Splitux;
use crate::input::{open_device, DeviceEvent, InputDevice};
use crate::monitor::get_monitors_sdl;
use eframe::egui::{self, RichText};
use egui_phosphor::regular as icons;
use std::time::Duration;

impl Splitux {
    pub fn spawn_task<F>(&mut self, msg: &str, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.loading_msg = Some(msg.to_string());
        self.loading_since = Some(std::time::Instant::now());
        self.task = Some(std::thread::spawn(f));
    }

    pub fn is_lite(&self) -> bool {
        self.handler_lite.is_some()
    }

    /// Poll for device hotplug events and update input_devices list
    pub(crate) fn poll_device_events(&mut self) {
        // Drain events first so the `device_monitor` borrow ends before we mutate
        // `input_devices` (which `reindex_instances_by_paths` reads).
        let events = match &mut self.device_monitor {
            Some(m) => m.poll_events(),
            None => return,
        };
        if events.is_empty() {
            return;
        }

        // Snapshot each seat's assigned devices BY PATH before we touch the device
        // list. `instance.devices` stores indices into `input_devices`, but a
        // hotplug push+sort (Add) or a mid-list remove (Remove) shifts those
        // indices — even a benign controller reconnect was enough to scramble a
        // live seat. We rebuild the indices from these stable paths afterward
        // instead of hand-patching them (the old decrement math only handled
        // Remove, and ignored the re-sort on Add entirely).
        let assigned_paths = self.snapshot_assigned_paths();

        for event in events {
            match event {
                DeviceEvent::Added(path) => {
                    // A duplicate Add for a device we already track is spurious:
                    // composite keyboards/mice/trackballs (ZSA Moonlander, Ploopy
                    // trackball) re-emit udev `add` events while live, and USB
                    // autosuspend/resume can too. A real reconnect arrives as Remove
                    // THEN Add; a bare Add for a known path is a no-op.
                    if self.input_devices.iter().any(|d| d.path() == path) {
                        continue;
                    }
                    println!("[splitux] udev: Add event for {}", path);
                    if let Some(device) = open_device(
                        &path,
                        &self.options.pad_filter_type,
                        &self.options.input_blacklist,
                    ) {
                        println!(
                            "[splitux] udev: Device connected: {} ({})",
                            device.fancyname(),
                            path
                        );
                        self.input_devices.push(device);
                        self.input_devices.sort_by_key(|d| d.path().to_string());
                    }
                }
                DeviceEvent::Removed(path) => {
                    if let Some(idx) = self.input_devices.iter().position(|d| d.path() == path) {
                        println!(
                            "[splitux] udev: Device disconnected: {} ({})",
                            self.input_devices[idx].fancyname(),
                            path
                        );
                        self.input_devices.remove(idx);
                    }
                }
            }
        }

        // Re-resolve every seat against the new device list by path and drop seats
        // whose every device unplugged. This is the SINGLE place indices are
        // rebuilt after a hotplug — no scattered decrement math to get wrong, and a
        // device that bounced Remove→Add within one poll keeps its seat assignment.
        self.reindex_instances_by_paths(assigned_paths);
        self.refresh_device_display_names();
    }

    /// Snapshot each instance's assigned devices as stable evdev paths.
    ///
    /// `instance.devices` holds indices into `input_devices`; those indices go
    /// stale whenever the list is re-sorted or re-sized (hotplug, rescan). Capture
    /// paths before such a mutation, then restore via [`Self::reindex_instances_by_paths`].
    fn snapshot_assigned_paths(&self) -> Vec<Vec<String>> {
        self.instances
            .iter()
            .map(|inst| {
                inst.devices
                    .iter()
                    .filter_map(|&i| self.input_devices.get(i).map(|d| d.path().to_string()))
                    .collect()
            })
            .collect()
    }

    /// Rebuild `instance.devices` indices from a path snapshot against the CURRENT
    /// `input_devices`, preserving per-seat order and dropping devices that no
    /// longer exist. Seats left with no devices are removed.
    fn reindex_instances_by_paths(&mut self, assigned_paths: Vec<Vec<String>>) {
        let path_to_idx: std::collections::HashMap<&str, usize> = self
            .input_devices
            .iter()
            .enumerate()
            .map(|(i, d)| (d.path(), i))
            .collect();
        for (inst, paths) in self.instances.iter_mut().zip(assigned_paths) {
            inst.devices = paths
                .iter()
                .filter_map(|p| path_to_idx.get(p.as_str()).copied())
                .collect();
        }
        self.instances.retain(|i| !i.devices.is_empty());
    }

    /// Replace the input-device list while keeping each seat pointed at the SAME
    /// physical devices. Use this for every full rescan (`scan_input_devices`) —
    /// the raw `self.input_devices = scan(...)` assignment re-sorts the list and
    /// silently invalidates the indices stored in `instance.devices`.
    pub(crate) fn set_input_devices(&mut self, new_devices: Vec<InputDevice>) {
        let assigned_paths = self.snapshot_assigned_paths();
        self.input_devices = new_devices;
        self.reindex_instances_by_paths(assigned_paths);
        self.refresh_device_display_names();
    }

    /// Poll for monitor changes (throttled to every 2 seconds)
    /// Similar to device hotplug but for display outputs
    pub(crate) fn poll_monitor_events(&mut self) {
        const POLL_INTERVAL: Duration = Duration::from_secs(2);

        if self.last_monitor_poll.elapsed() < POLL_INTERVAL {
            return;
        }
        self.last_monitor_poll = std::time::Instant::now();

        let current_monitors = get_monitors_sdl();

        // Check if monitors changed (different count or different properties)
        let changed = if current_monitors.len() != self.monitors.len() {
            true
        } else {
            current_monitors
                .iter()
                .zip(self.monitors.iter())
                .any(|(new, old)| {
                    new.name() != old.name()
                        || new.width() != old.width()
                        || new.height() != old.height()
                })
        };

        if changed {
            println!("[splitux] Monitor change detected:");
            for monitor in &current_monitors {
                println!(
                    "[splitux]   {} ({}x{})",
                    monitor.name(),
                    monitor.width(),
                    monitor.height()
                );
            }

            // Update instances if their monitor index is now out of bounds
            let max_monitor = current_monitors.len().saturating_sub(1);
            for instance in &mut self.instances {
                if instance.monitor > max_monitor {
                    println!(
                        "[splitux] Instance monitor {} out of bounds, resetting to {}",
                        instance.monitor, max_monitor
                    );
                    instance.monitor = max_monitor;
                }
            }

            self.monitors = current_monitors;
        }
    }

    /// Regenerate display names for all input devices (handles duplicates)
    pub fn refresh_device_display_names(&mut self) {
        self.device_display_names =
            crate::input::generate_display_names(&self.input_devices, &self.options.device_aliases);
    }

    /// Get display name for a device by index
    pub fn device_display_name(&self, idx: usize) -> &str {
        self.device_display_names
            .get(idx)
            .map(|s| s.as_str())
            .unwrap_or_else(|| {
                self.input_devices
                    .get(idx)
                    .map(|d| d.fancyname())
                    .unwrap_or("Unknown")
            })
    }

    /// Show permission warning banner if needed, returns true if banner was shown
    pub fn display_permission_banner(&mut self, ui: &mut egui::Ui) -> bool {
        // Don't show if dismissed or no permission issues
        if self.permission_banner_dismissed || self.permission_status.denied_count == 0 {
            return false;
        }

        let banner_color = egui::Color32::from_rgb(180, 120, 40); // Orange/amber warning
        egui::Frame::NONE
            .fill(banner_color.gamma_multiply(0.3))
            .stroke(egui::Stroke::new(1.0, banner_color))
            .corner_radius(4.0)
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icons::WARNING).size(18.0).color(banner_color));
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("Controller access requires setup")
                                .strong()
                                .color(egui::Color32::WHITE),
                        );
                        ui.label(
                            RichText::new(format!(
                                "{} input device(s) not accessible. Click 'Fix' to install udev rules.",
                                self.permission_status.denied_count
                            ))
                            .small()
                            .color(egui::Color32::LIGHT_GRAY),
                        );
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Dismiss button (use simple X for font compatibility)
                        if ui.small_button("X").on_hover_text("Dismiss").clicked() {
                            self.permission_banner_dismissed = true;
                        }

                        ui.add_space(8.0);

                        // Fix button - installs udev rules via pkexec
                        let fix_btn = ui.button("Fix Permissions");
                        if fix_btn
                            .on_hover_text("Install udev rules (requires password)")
                            .clicked()
                        {
                            println!("[splitux] Attempting to install udev rules via pkexec...");
                            match crate::input::install_udev_rules() {
                                Ok(true) => {
                                    println!("[splitux] Udev rules installed successfully");
                                    // Refresh permission status
                                    self.permission_status = crate::input::check_permissions();
                                    self.infotext =
                                        "Udev rules installed! Reconnect your controllers."
                                            .to_string();
                                }
                                Ok(false) => {
                                    println!("[splitux] User cancelled pkexec dialog");
                                    self.infotext = "Installation cancelled.".to_string();
                                }
                                Err(e) => {
                                    println!("[splitux] Failed to install udev rules: {}", e);
                                    self.infotext = format!("Failed: {}", e);
                                }
                            }
                        }
                    });
                });
            });

        ui.add_space(8.0);
        true
    }
}
