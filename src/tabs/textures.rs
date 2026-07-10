use crate::app::ResalinatedApp;
use eframe::egui;
use egui::{Color32, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use sas2_parser::xtexture::XSpriteRaw;

/// Phase 3: texture / cell metadata editor.
///
/// Left:   searchable list of textures from master.zcm.
/// Center: the sprite sheet with a cell-grid overlay; click a cell to select it.
/// Right:  numeric editor for the selected cell plus pixel actions (import / open externally).
pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    let Some(game_path) = app.game_path.clone() else {
        ui.label("Game folder not set. Set it in Settings to edit textures.");
        return;
    };

    app.texture_editor.ensure_loaded(&game_path);
    if app.texture_editor.master.is_none() {
        if let Some(s) = &app.texture_editor.status {
            ui.colored_label(Color32::RED, s);
        } else {
            ui.label("master.zcm not loaded.");
        }
        return;
    }

    // Keep the preview in sync with the selection and with external edits.
    app.texture_editor.ensure_sheet(ui.ctx(), &game_path);
    app.texture_editor.poll_external_edit(ui.ctx(), &game_path);

    let full_width = ui.available_width();

    // Right panel: cell editor + actions.
    egui::Panel::right("texture_cell_editor")
        .resizable(true)
        .default_size((full_width * 0.32).max(280.0))
        .min_size(260.0)
        .show_inside(ui, |ui| {
            show_cell_editor(app, ui, &game_path);
        });

    // Left panel: texture list.
    egui::Panel::left("texture_list")
        .resizable(true)
        .default_size(220.0)
        .min_size(160.0)
        .show_inside(ui, |ui| {
            show_texture_list(app, ui, &game_path);
        });

    // Center: sheet + overlay.
    egui::CentralPanel::default().show_inside(ui, |ui| {
        show_sheet_viewer(app, ui);
    });

    // Keep polling for external edits while this tab is open.
    if app.texture_editor.selected_texture.is_some() {
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(500));
    }
}

fn show_texture_list(app: &mut ResalinatedApp, ui: &mut Ui, game_path: &std::path::Path) {
    ui.heading("Textures");
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut app.texture_editor.search);
    });
    ui.horizontal(|ui| {
        if ui.button("New Texture").clicked() {
            app.texture_editor.create_blank_texture();
            app.texture_editor.search.clear();
        }
        let has_sel = app.texture_editor.selected_texture.is_some();
        if ui
            .add_enabled(has_sel, egui::Button::new("Clone Selected"))
            .on_hover_text("Duplicate the selected texture (cells + sheet) under a new name")
            .clicked()
        {
            if let Some(idx) = app.texture_editor.selected_texture {
                app.texture_editor.clone_texture(game_path, idx);
                app.texture_editor.search.clear();
            }
        }
        if ui
            .add_enabled(has_sel, egui::Button::new("Delete"))
            .on_hover_text("Remove the selected texture's cells and PNG override (save to apply)")
            .clicked()
        {
            if let Some(idx) = app.texture_editor.selected_texture {
                app.texture_editor.delete_texture(game_path, idx);
            }
        }
    });
    ui.separator();

    let indices = app.texture_editor.filtered_indices();
    let selected = app.texture_editor.selected_texture;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for idx in indices {
                let name = app.texture_editor.texture_name(idx);
                let cells = app
                    .texture_editor
                    .master
                    .as_ref()
                    .and_then(|m| m.entries.get(idx))
                    .map(|(_, t)| t.cells.iter().filter(|c| c.is_some()).count())
                    .unwrap_or(0);
                let label = format!("{}  ({} cells)", name, cells);
                if ui.selectable_label(selected == Some(idx), label).clicked() {
                    app.texture_editor.selected_texture = Some(idx);
                    app.texture_editor.selected_cell = None;
                }
            }
        });
}

fn show_sheet_viewer(app: &mut ResalinatedApp, ui: &mut Ui) {
    let te = &mut app.texture_editor;
    let Some(tex_idx) = te.selected_texture else {
        ui.label("Select a texture from the list.");
        return;
    };

    ui.horizontal(|ui| {
        ui.label(format!("Sheet: {}x{}", te.sheet_size.0, te.sheet_size.1));
        ui.separator();
        ui.label("Zoom:");
        ui.add(egui::Slider::new(&mut te.zoom, 0.1..=8.0).logarithmic(true));
        if ui.button("1:1").clicked() {
            te.zoom = 1.0;
        }
    });
    ui.separator();

    let Some(handle) = te.sheet_handle.clone() else {
        ui.colored_label(
            Color32::YELLOW,
            "No sprite sheet found for this texture (gfx/<name>.xnb missing). Cell rects can still be edited.",
        );
        return;
    };

    let zoom = te.zoom;
    let size = Vec2::new(te.sheet_size.0 as f32 * zoom, te.sheet_size.1 as f32 * zoom);

    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let (rect, response) = ui.allocate_exact_size(size, Sense::click());
            egui::Image::from_texture(&handle).paint_at(ui, rect);

            let painter = ui.painter_at(rect);
            let cells = te
                .master
                .as_ref()
                .and_then(|m| m.entries.get(tex_idx))
                .map(|(_, t)| t.cells.clone())
                .unwrap_or_default();

            // Map a source-space rect to screen-space.
            let to_screen = |x: f32, y: f32| Pos2::new(rect.min.x + x * zoom, rect.min.y + y * zoom);

            for (i, cell) in cells.iter().enumerate() {
                let Some(sprite) = cell else { continue };
                let (sx, sy, sw, sh) = sprite.src_rect;
                if sw <= 0 || sh <= 0 {
                    continue;
                }
                let cell_rect = Rect::from_min_max(
                    to_screen(sx as f32, sy as f32),
                    to_screen((sx + sw) as f32, (sy + sh) as f32),
                );
                let selected = te.selected_cell == Some(i);
                let stroke = if selected {
                    Stroke::new(2.0, Color32::YELLOW)
                } else {
                    Stroke::new(1.0, Color32::from_rgba_unmultiplied(0, 200, 255, 160))
                };
                painter.rect_stroke(cell_rect, 0.0, stroke, egui::StrokeKind::Middle);

                // Origin marker.
                let (ox, oy) = sprite.origin;
                let op = to_screen(ox, oy);
                let oc = if selected { Color32::YELLOW } else { Color32::from_rgb(255, 120, 0) };
                painter.line_segment([op - Vec2::new(4.0, 0.0), op + Vec2::new(4.0, 0.0)], Stroke::new(1.5, oc));
                painter.line_segment([op - Vec2::new(0.0, 4.0), op + Vec2::new(0.0, 4.0)], Stroke::new(1.5, oc));
            }

            // Click selects the topmost (smallest-area) cell under the pointer.
            if response.clicked() {
                if let Some(p) = response.interact_pointer_pos() {
                    let lx = (p.x - rect.min.x) / zoom;
                    let ly = (p.y - rect.min.y) / zoom;
                    let mut best: Option<(usize, i64)> = None;
                    for (i, cell) in cells.iter().enumerate() {
                        let Some(sprite) = cell else { continue };
                        let (sx, sy, sw, sh) = sprite.src_rect;
                        if sw <= 0 || sh <= 0 {
                            continue;
                        }
                        if lx >= sx as f32
                            && lx < (sx + sw) as f32
                            && ly >= sy as f32
                            && ly < (sy + sh) as f32
                        {
                            let area = sw as i64 * sh as i64;
                            if best.map_or(true, |(_, a)| area < a) {
                                best = Some((i, area));
                            }
                        }
                    }
                    if let Some((i, _)) = best {
                        te.selected_cell = Some(i);
                    }
                }
            }
        });
}

fn show_cell_editor(app: &mut ResalinatedApp, ui: &mut Ui, game_path: &std::path::Path) {
    let Some(tex_idx) = app.texture_editor.selected_texture else {
        ui.label("No texture selected.");
        return;
    };
    let name = app.texture_editor.texture_name(tex_idx);

    ui.heading("Texture");

    // Rename. Keep the buffer synced to the selection.
    if app.texture_editor.rename_for != Some(tex_idx) {
        app.texture_editor.rename_buffer = name.clone();
        app.texture_editor.rename_for = Some(tex_idx);
    }
    ui.horizontal(|ui| {
        ui.label("Name:");
        ui.text_edit_singleline(&mut app.texture_editor.rename_buffer);
        if ui.button("Rename").clicked() {
            let new_name = app.texture_editor.rename_buffer.clone();
            match app.texture_editor.rename_texture(game_path, tex_idx, &new_name) {
                Ok(()) => {
                    app.texture_editor.reload_sheet(ui.ctx(), game_path, tex_idx);
                    app.texture_editor.status = Some(format!("Renamed to '{}'", new_name));
                }
                Err(e) => app.texture_editor.status = Some(e),
            }
        }
    });
    if let Some(orig) = app.texture_editor.revert_name_target(tex_idx) {
        if ui
            .button(format!("Revert name to vanilla ('{}')", orig))
            .clicked()
        {
            match app.texture_editor.revert_name(game_path, tex_idx) {
                Ok(()) => {
                    app.texture_editor.reload_sheet(ui.ctx(), game_path, tex_idx);
                    app.texture_editor.status = Some(format!("Reverted name to '{}'", orig));
                }
                Err(e) => app.texture_editor.status = Some(e),
            }
        }
    }
    ui.add_space(4.0);

    // Pixel actions.
    ui.label("Pixels (PNG override):");
    ui.horizontal(|ui| {
        if ui
            .button("Import / Replace")
            .on_hover_text("Pick an image; it is copied to the config override as textures/<name>.png")
            .clicked()
        {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Image", &["png", "jpg", "jpeg", "bmp", "gif", "tga"])
                .pick_file()
            {
                match app.texture_editor.import_png(game_path, tex_idx, &path) {
                    Ok(()) => {
                        app.texture_editor.reload_sheet(ui.ctx(), game_path, tex_idx);
                        app.texture_editor.status = Some(format!("Imported override for '{}'", name));
                    }
                    Err(e) => app.texture_editor.status = Some(e),
                }
            }
        }
        if ui
            .button("Open in editor")
            .on_hover_text("Seed the override PNG from vanilla (if needed) and open it externally; changes auto-reload here. Configure the editor in Settings.")
            .clicked()
        {
            let editor = app.config.external_image_editor.clone();
            match app.texture_editor.open_external(game_path, tex_idx, &editor) {
                Ok(p) => app.texture_editor.status = Some(format!("Opened {}", p.display())),
                Err(e) => app.texture_editor.status = Some(e),
            }
        }
        if ui.button("Reload").clicked() {
            app.texture_editor.reload_sheet(ui.ctx(), game_path, tex_idx);
        }
    });

    // Reset is only meaningful for textures that exist in vanilla (not cloned/created ones).
    if app.texture_editor.is_vanilla_texture(tex_idx) {
        if ui
            .button("Reset to Vanilla")
            .on_hover_text("Restore vanilla cell metadata and delete the PNG override for this texture")
            .clicked()
        {
            match app.texture_editor.reset_texture_to_vanilla(game_path, tex_idx) {
                Ok(()) => {
                    app.texture_editor.reload_sheet(ui.ctx(), game_path, tex_idx);
                    app.texture_editor.status = Some(format!("Reset '{}' to vanilla", name));
                }
                Err(e) => app.texture_editor.status = Some(e),
            }
        }
    }

    ui.separator();

    // Cell editor.
    ui.heading("Cell");
    let Some(cell_idx) = app.texture_editor.selected_cell else {
        ui.label("Click a cell in the sheet to edit it.");
        show_save_row(app, ui, game_path);
        return;
    };

    ui.label(format!("Index: {}", cell_idx));

    let mut dirty = false;
    let mut remove = false;
    let mut add = false;

    if let Some(master) = app.texture_editor.master.as_mut() {
        if let Some((_, tex)) = master.entries.get_mut(tex_idx) {
            match tex.cells.get_mut(cell_idx) {
                Some(Some(cell)) => {
                    dirty |= cell_fields(ui, cell);
                    ui.add_space(4.0);
                    if ui.button("Remove sprite (set cell empty)").clicked() {
                        remove = true;
                    }
                }
                Some(None) => {
                    ui.label("This cell is empty.");
                    if ui.button("Add sprite here").clicked() {
                        add = true;
                    }
                }
                None => {
                    ui.colored_label(Color32::RED, "Cell index out of range.");
                }
            }
            if remove {
                tex.cells[cell_idx] = None;
                dirty = true;
            } else if add {
                tex.cells[cell_idx] = Some(XSpriteRaw::default());
                dirty = true;
            }
        }
    }

    if dirty {
        app.texture_editor.dirty = true;
    }

    show_save_row(app, ui, game_path);
}

/// Numeric editors for one sprite cell. Returns true if any value changed.
fn cell_fields(ui: &mut Ui, cell: &mut XSpriteRaw) -> bool {
    let mut changed = false;
    egui::Grid::new("cell_fields").num_columns(2).show(ui, |ui| {
        ui.label("src x");
        changed |= ui.add(egui::DragValue::new(&mut cell.src_rect.0)).changed();
        ui.end_row();
        ui.label("src y");
        changed |= ui.add(egui::DragValue::new(&mut cell.src_rect.1)).changed();
        ui.end_row();
        ui.label("width");
        changed |= ui.add(egui::DragValue::new(&mut cell.src_rect.2)).changed();
        ui.end_row();
        ui.label("height");
        changed |= ui.add(egui::DragValue::new(&mut cell.src_rect.3)).changed();
        ui.end_row();
        ui.label("origin x");
        changed |= ui
            .add(egui::DragValue::new(&mut cell.origin.0).speed(0.5))
            .changed();
        ui.end_row();
        ui.label("origin y");
        changed |= ui
            .add(egui::DragValue::new(&mut cell.origin.1).speed(0.5))
            .changed();
        ui.end_row();
    });
    changed
}

fn show_save_row(app: &mut ResalinatedApp, ui: &mut Ui, game_path: &std::path::Path) {
    ui.separator();
    ui.horizontal(|ui| {
        let dirty = app.texture_editor.dirty;
        if ui
            .add_enabled(dirty, egui::Button::new("Save Cell Metadata"))
            .on_hover_text("Write master.zcm into the config override the loader reads")
            .clicked()
        {
            match app.texture_editor.save_master(game_path) {
                Ok(()) => app.texture_editor.status = Some("Saved master.zcm override".to_string()),
                Err(e) => app.texture_editor.status = Some(e),
            }
        }
        if dirty {
            ui.colored_label(Color32::YELLOW, "unsaved");
        }
    });
    if let Some(s) = &app.texture_editor.status {
        ui.add_space(2.0);
        ui.colored_label(Color32::LIGHT_BLUE, s);
    }
}
