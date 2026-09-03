use crate::app::ResalinatedApp;
use eframe::egui;
use egui::{Color32, Ui};

/// Images tab: import custom item icons into a separate custom atlas.
/// An item uses one by setting its Img field to the shown value (vanilla capacity + the icon's local index); the loader draws it from the custom atlas.
/// Icons are stored in the working folder, saved into presets, and composited into the live config on Apply.
pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    let Some(game_path) = app.game_path.clone() else {
        ui.label("Game folder not set. Set it in Settings to manage icons.");
        return;
    };

    app.image_editor.ensure_loaded(ui.ctx(), &game_path);

    ui.heading("Custom item icons");
    ui.label(
        "Custom icons live in their own atlas. \
        Set an item's Img field to the value shown under each icon. \
        Icons are saved with the preset and composited on Apply.",
    );

    ui.horizontal(|ui| {
        let can_import = app.image_editor.next_free_local().is_some();
        if ui
            .add_enabled(can_import, egui::Button::new("Import Icon"))
            .on_hover_text("Pick a PNG (resized to 128x128); it takes the next free custom slot")
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Image", &["png", "jpg", "jpeg", "bmp", "gif", "tga"])
                .pick_file()
            {
                match app.image_editor.import_icon(ui.ctx(), &game_path, &path) {
                    Ok(local) => {
                        let img = app.image_editor.global_img(local);
                        app.image_editor.status =
                            Some(format!("Imported custom icon (set item Img = {})", img));
                    }
                    Err(e) => app.image_editor.status = Some(e),
                }
            }
        }
    });

    if let Some(s) = &app.image_editor.status {
        ui.colored_label(Color32::LIGHT_BLUE, s);
    }
    ui.separator();

    if app.image_editor.icons.is_empty() {
        ui.label("No custom icons yet. Import one to get started.");
        return;
    }

    let icon_size = 96.0;
    let mut to_delete: Option<i32> = None;
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show_viewport(ui, |ui, viewport| {
            // Only rows whose y-range intersects the visible viewport are laid out each frame.
            // Culled cells still advance the grid cursor via allocate_space with the exact cell size, so positions, row heights and the scrollbar stay exact while the widget count stays proportional to the viewport.
            let label_h = ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(12.0)));
            let spacing_y = ui.spacing().item_spacing.y;
            let pad_x = 2.0 * ui.spacing().button_padding.x;
            let pad_y = 2.0 * ui.spacing().button_padding.y;
            let overscan = 3.0 * (icon_size + pad_x);
            let vp_min = viewport.min.y - overscan;
            let vp_max = viewport.max.y + overscan;

            egui::Grid::new("custom_icons")
                .spacing([12.0, 12.0])
                .show(ui, |ui| {
                    let cap = app.image_editor.capacity as i32;
                    let mut col = 0;
                    let mut row_y = 0.0f32;
                    let mut row_h = 0.0f32;
                    for (local, handle) in &app.image_editor.icons {
                        let item_w = icon_size + pad_x;
                        let item_h = icon_size + pad_y + label_h + spacing_y;
                        row_h = row_h.max(item_h);
                        let row_visible = row_y + row_h >= vp_min && row_y <= vp_max;
                        if !row_visible {
                            ui.allocate_space(egui::vec2(item_w, item_h));
                        } else {
                            ui.vertical(|ui| {
                                ui.add(
                                    egui::Image::from_texture(handle)
                                        .fit_to_exact_size(egui::vec2(icon_size, icon_size)),
                                );
                                ui.label(format!("Img = {}", cap + *local));
                                if ui.small_button("Delete").clicked() {
                                    to_delete = Some(*local);
                                }
                            });
                        }
                        col += 1;
                        if col % 6 == 0 {
                            ui.end_row();
                            row_y += row_h + spacing_y;
                            row_h = 0.0;
                        }
                    }
                });
        });

    if let Some(local) = to_delete {
        app.image_editor.delete_icon(ui.ctx(), &game_path, local);
        app.image_editor.status = Some("Deleted custom icon".to_string());
    }
}
