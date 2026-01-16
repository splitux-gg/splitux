//! Profile Builder settings page - KB/Mouse Mapper
//!
//! Simple interface: controller diagram for mapping, name input, basic config.
//! Supports full gamepad navigation.

use super::Splitux;
use crate::app::app::ProfileBuilderFocus;
use crate::gptokeyb::{
    delete_profile, list_user_profiles, load_user_profile, save_profile, AnalogMode,
    GptokeybProfile,
};
use crate::ui::components::controller_diagram::{
    render_button_legend, render_controller_diagram, DIAGRAM_BUTTONS,
};
use crate::ui::theme;
use crate::util::msg;
use eframe::egui::{self, RichText, StrokeKind, Ui};
use egui_phosphor::regular as icons;

impl Splitux {
    pub fn display_settings_profile_builder(&mut self, ui: &mut Ui) {
        if let Some(ref profile) = self.profile_builder_editing.clone() {
            self.display_profile_editor(ui, profile);
        } else {
            self.display_profile_list(ui);
        }
    }

    fn display_profile_list(&mut self, ui: &mut Ui) {
        let focus = self.profile_builder_focus;

        ui.horizontal(|ui| {
            ui.heading("KB/Mouse Profiles");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let is_focused = focus == ProfileBuilderFocus::NewButton;
                let btn = egui::Button::new(format!("{} New", icons::PLUS));
                let btn = if is_focused {
                    btn.fill(theme::colors::ACCENT_DIM)
                } else {
                    btn
                };
                if ui.add(btn).clicked() {
                    self.create_new_profile();
                }
            });
        });
        ui.add_space(8.0);

        if self.profile_builder_profiles.is_empty() {
            ui.label(RichText::new("No custom profiles yet.").weak());
            ui.label(RichText::new("Press A or click New to create a profile.").weak().small());
        } else {
            for (idx, name) in self.profile_builder_profiles.clone().iter().enumerate() {
                let row_focused = matches!(focus, ProfileBuilderFocus::ProfileRow(i, _) if i == idx);
                let edit_focused = matches!(focus, ProfileBuilderFocus::ProfileRow(i, 1) if i == idx);
                let delete_focused = matches!(focus, ProfileBuilderFocus::ProfileRow(i, 2) if i == idx);

                let frame = if row_focused {
                    egui::Frame::NONE
                        .fill(theme::colors::ACCENT_DIM)
                        .inner_margin(4.0)
                        .corner_radius(4.0)
                } else {
                    egui::Frame::NONE.inner_margin(4.0)
                };

                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(name);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let del_btn = egui::Button::new(icons::TRASH).small();
                            let del_btn = if delete_focused {
                                del_btn.fill(theme::colors::SURFACE_DESTRUCTIVE)
                            } else {
                                del_btn
                            };
                            if ui.add(del_btn).clicked() {
                                let _ = delete_profile(name);
                                self.profile_builder_profiles = list_user_profiles();
                            }

                            let edit_btn = egui::Button::new(icons::PENCIL_SIMPLE).small();
                            let edit_btn = if edit_focused {
                                edit_btn.fill(theme::colors::ACCENT)
                            } else {
                                edit_btn
                            };
                            if ui.add(edit_btn).clicked() {
                                self.edit_profile(name);
                            }
                        });
                    });
                });
            }
        }
    }

    fn display_profile_editor(&mut self, ui: &mut Ui, profile: &GptokeybProfile) {
        let mut profile = profile.clone();
        let focus = self.profile_builder_focus;

        // Header row
        ui.horizontal(|ui| {
            let name_focused = focus == ProfileBuilderFocus::NameInput;
            ui.label("Name:");
            let text_edit = egui::TextEdit::singleline(&mut self.profile_builder_name_buffer)
                .desired_width(120.0);
            let resp = ui.add(text_edit);
            if name_focused {
                // Draw focus ring around text edit
                let rect = resp.rect.expand(2.0);
                ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(2.0, theme::colors::ACCENT), StrokeKind::Inside);
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let cancel_focused = focus == ProfileBuilderFocus::CancelButton;
                let cancel_btn = egui::Button::new("Cancel");
                let cancel_btn = if cancel_focused {
                    cancel_btn.fill(theme::colors::SURFACE_DESTRUCTIVE)
                } else {
                    cancel_btn
                };
                if ui.add(cancel_btn).clicked() {
                    self.cancel_editor();
                }

                let save_focused = focus == ProfileBuilderFocus::SaveButton;
                let save_btn = egui::Button::new(format!("{} Save", icons::FLOPPY_DISK));
                let save_btn = if save_focused {
                    save_btn.fill(theme::colors::ACCENT)
                } else {
                    save_btn
                };
                if ui.add(save_btn).clicked() {
                    self.save_current_profile(&profile);
                }
            });
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        // Controller diagram
        let in_diagram = matches!(focus, ProfileBuilderFocus::DiagramButton(_));
        render_button_legend(ui, in_diagram);
        ui.add_space(4.0);

        // Get gamepad-focused button for diagram
        let gamepad_focused = if let ProfileBuilderFocus::DiagramButton(idx) = focus {
            DIAGRAM_BUTTONS.get(idx).copied()
        } else {
            None
        };

        let profile_ref = &profile;
        let response = render_controller_diagram(
            ui,
            self.profile_builder_selected_button,
            gamepad_focused,
            |btn| profile_ref.get_mapping(btn).map(|s| s.to_string()),
        );

        if let Some(btn) = response.clicked {
            self.profile_builder_selected_button = Some(btn);
        }

        // Selected button mapping
        if let Some(selected) = self.profile_builder_selected_button {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(selected.display_name()).strong());

                let input_focused = focus == ProfileBuilderFocus::MappingInput;
                let current = profile.get_mapping(selected).unwrap_or("").to_string();
                let mut input = current.clone();
                let text_edit = egui::TextEdit::singleline(&mut input)
                    .desired_width(100.0)
                    .hint_text("key");
                let resp = ui.add(text_edit);
                if input_focused {
                    let rect = resp.rect.expand(2.0);
                    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(2.0, theme::colors::ACCENT), StrokeKind::Inside);
                }
                if resp.changed() {
                    if input.is_empty() {
                        profile.clear_mapping(selected);
                    } else {
                        profile.set_mapping(selected, &input);
                    }
                }

                let clear_focused = focus == ProfileBuilderFocus::ClearMapping;
                let clear_btn = egui::Button::new(icons::X).small();
                let clear_btn = if clear_focused {
                    clear_btn.fill(theme::colors::SURFACE_DESTRUCTIVE)
                } else {
                    clear_btn
                };
                if ui.add(clear_btn).clicked() {
                    profile.clear_mapping(selected);
                }
            });
            ui.label(
                RichText::new("w/a/s/d, space, ctrl, shift, mouse_left, mouse_right, mouse_wheel_up/down")
                    .weak()
                    .small(),
            );
        }

        ui.add_space(12.0);
        ui.separator();
        ui.add_space(8.0);

        // Config section
        ui.horizontal(|ui| {
            let right_focused = focus == ProfileBuilderFocus::RightStickMouse;
            if right_focused {
                ui.label(RichText::new("Right stick mouse:").color(theme::colors::ACCENT));
            } else {
                ui.label("Right stick mouse:");
            }
            let mut mouse = profile.right_analog_mode == AnalogMode::MouseMovement;
            let checkbox = egui::Checkbox::new(&mut mouse, "");
            let resp = ui.add(checkbox);
            if right_focused {
                let rect = resp.rect.expand(2.0);
                ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(2.0, theme::colors::ACCENT), StrokeKind::Inside);
            }
            if resp.changed() {
                profile.right_analog_mode = if mouse {
                    AnalogMode::MouseMovement
                } else {
                    AnalogMode::Disabled
                };
            }

            ui.add_space(16.0);

            let left_focused = focus == ProfileBuilderFocus::LeftStickMouse;
            if left_focused {
                ui.label(RichText::new("Left stick mouse:").color(theme::colors::ACCENT));
            } else {
                ui.label("Left stick mouse:");
            }
            let mut lmouse = profile.left_analog_mode == AnalogMode::MouseMovement;
            let checkbox = egui::Checkbox::new(&mut lmouse, "");
            let resp = ui.add(checkbox);
            if left_focused {
                let rect = resp.rect.expand(2.0);
                ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(2.0, theme::colors::ACCENT), StrokeKind::Inside);
            }
            if resp.changed() {
                profile.left_analog_mode = if lmouse {
                    AnalogMode::MouseMovement
                } else {
                    AnalogMode::Disabled
                };
            }
        });

        ui.horizontal(|ui| {
            let speed_focused = focus == ProfileBuilderFocus::MouseSpeed;
            if speed_focused {
                ui.label(RichText::new("Mouse speed:").color(theme::colors::ACCENT));
            } else {
                ui.label("Mouse speed:");
            }
            let mut scale = profile.config.mouse_scale as f32;
            let slider = egui::Slider::new(&mut scale, 10.0..=150.0);
            let resp = ui.add(slider);
            if speed_focused {
                let rect = resp.rect.expand(2.0);
                ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(2.0, theme::colors::ACCENT), StrokeKind::Inside);
            }
            if resp.changed() {
                profile.config.mouse_scale = scale as u32;
            }
        });

        self.profile_builder_editing = Some(profile);
    }

    // === Helper methods for actions ===

    fn create_new_profile(&mut self) {
        let mut profile = GptokeybProfile::new("my_profile");
        profile.right_analog_mode = AnalogMode::MouseMovement;
        self.profile_builder_editing = Some(profile);
        self.profile_builder_name_buffer = "my_profile".to_string();
        self.profile_builder_focus = ProfileBuilderFocus::NameInput;
    }

    fn edit_profile(&mut self, name: &str) {
        if let Ok(profile) = load_user_profile(name) {
            self.profile_builder_name_buffer = profile.name.clone();
            self.profile_builder_editing = Some(profile);
            self.profile_builder_focus = ProfileBuilderFocus::NameInput;
        }
    }

    fn cancel_editor(&mut self) {
        self.profile_builder_editing = None;
        self.profile_builder_selected_button = None;
        self.profile_builder_focus = ProfileBuilderFocus::NewButton;
    }

    fn save_current_profile(&mut self, profile: &GptokeybProfile) {
        let name = self.profile_builder_name_buffer.trim();
        if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-') {
            let mut p = profile.clone();
            p.name = name.to_string();
            if save_profile(&p).is_ok() {
                self.profile_builder_editing = None;
                self.profile_builder_selected_button = None;
                self.profile_builder_profiles = list_user_profiles();
                self.profile_builder_focus = ProfileBuilderFocus::NewButton;
            }
        } else {
            msg("Error", "Invalid profile name (use letters, numbers, _ or -)");
        }
    }

}
