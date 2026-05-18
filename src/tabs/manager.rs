use crate::app::ResalinatedApp;
use eframe::egui;
use egui::Ui;
use std::time::Instant;

pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    let full_width = ui.available_width();
    let min_size = 200.0;
    let left_width = if app.config.manager_left_panel_width > 0.0 {
        app.config.manager_left_panel_width.max(min_size)
    } else {
        full_width * 0.4
    };

    // Left panel: available presets
    let left_panel = egui::Panel::left("available_presets")
        .resizable(true)
        .default_size(left_width)
        .min_size(min_size)
        .max_size(full_width * 0.6)
        .show_inside(ui, |ui| {
            ui.heading("Available Presets");
            let available: Vec<String> = app
                .preset_manager
                .installed_presets()
                .iter()
                .filter(|p| {
                    !app.preset_manager
                        .enabled_presets()
                        .contains(&p.folder_name)
                })
                .map(|p| p.folder_name.clone())
                .collect();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for (i, name) in available.iter().enumerate() {
                    if ui
                        .selectable_label(app.manager_selected_available == Some(i), name)
                        .clicked()
                    {
                        app.manager_selected_available = Some(i);
                    }
                }
            });

            if !available.is_empty() {
                ui.horizontal(|ui| {
                    if ui.button("Enable").clicked() {
                        if let Some(idx) = app.manager_selected_available {
                            let name = available[idx].clone();
                            app.preset_manager.enable_preset(&name);
                            app.manager_selected_available = None;
                            app.manager_selected_enabled = None;
                        }
                    }
                    if ui.button("Edit").clicked() {
                        if let Some(idx) = app.manager_selected_available {
                            let name = available[idx].clone();
                            app.load_preset(&name);
                        }
                    }
                    if ui.button("Delete").clicked() {
                        if let Some(idx) = app.manager_selected_available {
                            let name = available[idx].clone();
                            if let Err(e) = app.preset_manager.delete_preset(&name) {
                                app.error_message = Some(e);
                            } else {
                                app.error_message = None;
                                app.manager_selected_available = None;
                            }
                        }
                    }
                    if app.manager_selected_available.is_some() {
                        if ui.button("Export").clicked() {
                            let name = available[app.manager_selected_available.unwrap()].clone();
                            if let Some(path) = rfd::FileDialog::new()
                                .set_file_name(&format!("{}.zip", name))
                                .save_file()
                            {
                                if let Err(e) = app.preset_manager.export_preset(&name, &path) {
                                    app.error_message = Some(e);
                                } else {
                                    app.error_message = None;
                                }
                            }
                        }
                    }
                });
            }

            if ui.button("Import Preset").clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("Zip archive", &["zip"])
                    .pick_file()
                {
                    if let Err(e) = app.preset_manager.import_preset(&path) {
                        app.error_message = Some(e);
                    } else {
                        app.error_message = None;
                    }
                }
            }
        });

    // Save left panel width
    let actual_left = left_panel.response.rect.width();
    if (actual_left - app.config.manager_left_panel_width).abs() > 0.1 {
        app.config.manager_left_panel_width = actual_left;
        app.config_save_timer = 0.25;
    }

    // Right panel: enabled presets
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.heading("Enabled Presets (ordered)");

        ui.horizontal(|ui| {
            if ui.button("⟳ Apply Now").clicked() {
                app.apply_enabled_presets();
                app.apply_feedback_time = Some(Instant::now());
            }

            if let Some(click_time) = app.apply_feedback_time {
                if click_time.elapsed().as_secs_f32() < 2.0 {
                    ui.colored_label(egui::Color32::GREEN, "Applied");
                } else {
                    app.apply_feedback_time = None;
                    ui.label("Write merged catalog to game folder");
                }
            } else {
                ui.label("Write merged catalog to game folder");
            }
        });

        let enabled = app.preset_manager.enabled_presets().to_vec();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for (i, name) in enabled.iter().enumerate() {
                if ui
                    .selectable_label(app.manager_selected_enabled == Some(i), name)
                    .clicked()
                {
                    app.manager_selected_enabled = Some(i);
                }
            }
        });

        if !enabled.is_empty() {
            ui.horizontal(|ui| {
                if ui.button("< Disable").clicked() {
                    if let Some(idx) = app.manager_selected_enabled {
                        let name = enabled[idx].clone();
                        if let Err(e) = app.preset_manager.disable_preset(&name) {
                            app.error_message = Some(e);
                        } else {
                            app.error_message = None;
                        }
                        app.manager_selected_enabled = None;
                    }
                }
                if ui.button("Edit").clicked() {
                    if let Some(idx) = app.manager_selected_enabled {
                        let name = enabled[idx].clone();
                        app.load_preset(&name);
                    }
                }
                if app.manager_selected_enabled.is_some() {
                    if ui.button("Export").clicked() {
                        let name = enabled[app.manager_selected_enabled.unwrap()].clone();
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name(&format!("{}.zip", name))
                            .save_file()
                        {
                            if let Err(e) = app.preset_manager.export_preset(&name, &path) {
                                app.error_message = Some(e);
                            } else {
                                app.error_message = None;
                            }
                        }
                    }
                }
                if ui.button("Up").clicked() {
                    if let Some(idx) = app.manager_selected_enabled {
                        if idx > 0 {
                            app.preset_manager.move_preset(idx, idx - 1);
                            app.manager_selected_enabled = Some(idx - 1);
                        }
                    }
                }
                if ui.button("Down").clicked() {
                    if let Some(idx) = app.manager_selected_enabled {
                        if idx + 1 < enabled.len() {
                            app.preset_manager.move_preset(idx, idx + 1);
                            app.manager_selected_enabled = Some(idx + 1);
                        }
                    }
                }
            });
            // Add Delete button (only for non-vanilla)
            let is_vanilla = if let Some(idx) = app.manager_selected_enabled {
                &enabled[idx] == "Vanilla (Base)"
            } else {
                false
            };
            if !is_vanilla {
                if ui.button("Delete").clicked() {
                    if let Some(idx) = app.manager_selected_enabled {
                        let name = enabled[idx].clone();
                        if let Err(e) = app.preset_manager.delete_preset(&name) {
                            app.error_message = Some(e);
                        } else {
                            app.error_message = None;
                            app.manager_selected_enabled = None;
                        }
                    }
                }
            }
        }
    });
}
