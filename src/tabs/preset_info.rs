use crate::app::ResalinatedApp;
use eframe::egui;
use egui::Ui;

pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    ui.heading("Preset Information");
    ui.separator();

    ui.horizontal(|ui| {
        ui.label("Folder Name:");
    });
    ui.text_edit_singleline(&mut app.edit_folder_name);

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
        if ui.button("Save Modified as Preset").clicked() {
            if app.edit_folder_name.is_empty() {
                app.error_message = Some("Folder name cannot be empty".to_string());
            } else {
                let folder_name = app.edit_folder_name.clone();
                let meta = app.edit_meta.clone();
                if app
                    .preset_manager
                    .installed_presets()
                    .iter()
                    .any(|p| p.folder_name == folder_name)
                {
                    app.confirm_overwrite_folder = Some(folder_name);
                } else {
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
    }
}
