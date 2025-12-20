//! Helper methods for Splitux

use super::Splitux;
use crate::input::{open_device, DeviceEvent};
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
        let monitor = match &mut self.device_monitor {
            Some(m) => m,
            None => return,
        };

        for event in monitor.poll_events() {
            match event {
                DeviceEvent::Added(path) => {
                    println!("[splitux] udev: Add event for {}", path);
                    // Remove any stale entry with the same path first
                    if let Some(idx) = self.input_devices.iter().position(|d| d.path() == path) {
                        println!("[splitux] udev: Removing stale entry for {}", path);
                        // Clean up instances referencing this device
                        for instance in &mut self.instances {
                            instance.devices.retain(|&d| d != idx);
                        }
                        self.instances.retain(|i| !i.devices.is_empty());
                        for instance in &mut self.instances {
                            for dev_idx in &mut instance.devices {
                                if *dev_idx > idx {
                                    *dev_idx -= 1;
                                }
                            }
                        }
                        self.input_devices.remove(idx);
                    }
                    // Try to open the device
                    if let Some(device) = open_device(&path, &self.options.pad_filter_type) {
                        println!(
                            "[splitux] udev: Device connected: {} ({})",
                            device.fancyname(),
                            path
                        );
                        self.input_devices.push(device);
                        self.input_devices.sort_by_key(|d| d.path().to_string());
                        self.refresh_device_display_names();
                    }
                }
                DeviceEvent::Removed(path) => {
                    // Find and remove the device
                    if let Some(idx) = self.input_devices.iter().position(|d| d.path() == path) {
                        let device = &self.input_devices[idx];
                        println!(
                            "[splitux] udev: Device disconnected: {} ({})",
                            device.fancyname(),
                            path
                        );

                        // Also remove from any instances
                        for instance in &mut self.instances {
                            instance.devices.retain(|&d| d != idx);
                        }
                        // Remove empty instances
                        self.instances.retain(|i| !i.devices.is_empty());
                        // Update device indices in instances (since we're removing one)
                        for instance in &mut self.instances {
                            for dev_idx in &mut instance.devices {
                                if *dev_idx > idx {
                                    *dev_idx -= 1;
                                }
                            }
                        }

                        self.input_devices.remove(idx);
                        self.refresh_device_display_names();
                    }
                }
            }
        }
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
