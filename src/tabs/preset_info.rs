use crate::app::ResalinatedApp;
use crate::preset::guid_folder_name;
use eframe::egui;
use egui::Ui;

pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    ui.heading("Preset Information");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Folder Name:");
        ui.add_enabled(
            app.folder_override_enabled,
            egui::TextEdit::singleline(&mut app.edit_folder_name),
        );
        if app.folder_override_enabled {
            if ui
                .button("Use GUID")
                .on_hover_text("Back to the automatic GUID folder name")
                .clicked()
            {
                app.folder_override_enabled = false;
                app.edit_folder_name = guid_folder_name(&app.edit_meta);
            }
        } else if ui
            .button("Override Folder (not recommended)")
            .on_hover_text("Manually choose the folder name. Two presets with the same folder name would overwrite each other.")
            .clicked()
        {
            app.folder_override_enabled = true;
        }
    });
    app.edit_meta.folder_override = app.folder_override_enabled;
    ui.label(
        egui::RichText::new(format!(
            "Full path: {}",
            app.preset_folder_path().display()
        ))
        .small()
        .weak(),
    );

    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut app.edit_meta.name);
    });
    ui.horizontal(|ui| {
        ui.label("Version:");
        ui.text_edit_singleline(&mut app.edit_meta.version);
    });
    ui.horizontal(|ui| {
        ui.label("Author:");
        ui.text_edit_singleline(&mut app.edit_meta.author);
    });
    ui.horizontal(|ui| {
        ui.label("Description:");
        ui.text_edit_singleline(&mut app.edit_meta.description);
    });

    ui.separator();

    ui.horizontal(|ui| {
        if ui
            .checkbox(
                &mut app.config.ignore_overwrite_warning,
                "Ignore overwrite warning (always overwrite)",
            )
            .changed()
        {
            app.config_save_timer = 0.1;
        }
    });

    // Overwrite confirmation, only clear when user explicitly answers
    if let Some(ref existing_folder) = app.confirm_overwrite_folder.clone() {
        let overwrite_name = existing_folder.clone();
        ui.colored_label(
            egui::Color32::RED,
            format!("Preset '{}' already exists. Overwrite?", overwrite_name),
        );
        ui.horizontal(|ui| {
            if ui.button("Yes, overwrite").clicked() {
                let folder = overwrite_name.clone();
                let meta = app.edit_meta.clone();
                let _ = app.preset_manager.delete_preset(&folder);
                match app.save_preset(&folder, meta) {
                    Ok(()) => {
                        app.error_message = None;
                        app.active_tab = crate::tabs::Tab::Manager;
                        app.confirm_overwrite_folder = None;
                    }
                    Err(e) => {
                        app.error_message = Some(e);
                        app.confirm_overwrite_folder = None;
                    }
                }
            }
            if ui.button("No").clicked() {
                app.confirm_overwrite_folder = None;
            }
        });
    } else {
        let is_vanilla = app.edit_folder_name == "Vanilla (Base)";
        let target_folder = if app.folder_override_enabled {
            app.edit_folder_name.clone()
        } else {
            guid_folder_name(&app.edit_meta)
        };
        // "Save as new preset" only makes sense when the identity changed (author/name/version or override folder), or when no preset is loaded.
        let can_save_as_new =
            app.edit_folder_name.is_empty() || target_folder != app.edit_folder_name;
        ui.horizontal(|ui| {
            let save_resp = ui.add_enabled(!is_vanilla, egui::Button::new("Save"));
            if save_resp
                .on_hover_text(if is_vanilla {
                    "The Vanilla preset cannot be modified"
                } else {
                    "Save changes to this preset. If you renamed author/name/version, the folder is moved to the new GUID name."
                })
                .clicked()
            {
                match app.save_preset_in_place() {
                    Ok(_) => {
                        app.error_message = None;
                        app.save_feedback_time = Some(std::time::Instant::now());
                    }
                    Err(e) => app.error_message = Some(e),
                }
            }
            let save_as_new_resp = ui.add_enabled(can_save_as_new, egui::Button::new("Save as new preset"));
            if save_as_new_resp
                .on_hover_text(if can_save_as_new {
                    "Save the current changes as a new preset, keeping the original untouched"
                } else {
                    "Rename author, name or version to save as a new preset"
                })
                .clicked()
            {
                let folder_name = target_folder;
                if folder_name.is_empty() {
                    app.error_message = Some("Folder name cannot be empty".to_string());
                } else {
                    let meta = app.edit_meta.clone();
                    let exists = app
                        .preset_manager
                        .installed_presets()
                        .iter()
                        .any(|p| p.folder_name == folder_name);
                    if exists && !app.config.ignore_overwrite_warning {
                        app.confirm_overwrite_folder = Some(folder_name);
                    } else {
                        if exists {
                            let _ = app.preset_manager.delete_preset(&folder_name);
                        }
                        match app.save_preset(&folder_name, meta) {
                            Ok(()) => {
                                app.error_message = None;
                                app.active_tab = crate::tabs::Tab::Manager;
                            }
                            Err(e) => app.error_message = Some(e),
                        }
                    }
                }
            }
            if !app.edit_folder_name.is_empty() {
                if ui.button("Export").clicked() {
                    let name = app.edit_folder_name.clone();
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
        if let Some(click_time) = app.save_feedback_time {
            if click_time.elapsed().as_secs_f32() < 2.0 {
                ui.colored_label(egui::Color32::GREEN, "Saved");
            } else {
                app.save_feedback_time = None;
            }
        }
    }
}
