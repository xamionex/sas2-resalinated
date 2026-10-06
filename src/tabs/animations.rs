use crate::app::ResalinatedApp;
use eframe::egui;
use egui::{Color32, Pos2, Rect, Stroke, Ui};
use sas2_parser::char_def::{KeyFrame, Part};
use std::path::Path;

/// Phase 4: animation timeline editor for character defs (.zsx).
pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    let Some(game_path) = app.game_path.clone() else {
        ui.label("Game folder not set. Set it in Settings to edit animations.");
        return;
    };

    app.anim_editor.ensure_files(&game_path);

    egui::Panel::left("anim_left")
        .resizable(true)
        .default_size(240.0)
        .min_size(170.0)
        .show(ui, |ui| {
            show_left(app, ui, &game_path);
        });

    if app.anim_editor.char_def.is_none() {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.label("Select a character def to edit its animations.");
            if let Some(s) = &app.anim_editor.status {
                ui.colored_label(Color32::LIGHT_BLUE, s);
            }
        });
        return;
    }

    egui::Panel::right("anim_inspector")
        .resizable(true)
        .default_size(320.0)
        .min_size(260.0)
        .show(ui, |ui| {
            show_inspector(app, ui, &game_path);
        });

    egui::CentralPanel::default().show(ui, |ui| {
        show_preview_and_timeline(app, ui);
    });

    // Drive playback.
    if app.anim_editor.playing {
        let dt = ui.input(|i| i.stable_dt);
        app.anim_editor.advance_playback(dt);
        ui.ctx().request_repaint();
    }
}

fn show_left(app: &mut ResalinatedApp, ui: &mut Ui, game_path: &Path) {
    ui.heading("Characters");
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(&mut app.anim_editor.file_search);
    });

    let files = app.anim_editor.filtered_files();
    let loaded = app.anim_editor.loaded_stem.clone();
    // Virtualized rows: only the rows visible in the scroll viewport are laid out each frame.
    let row_height = ui
        .text_style_height(&egui::TextStyle::Body)
        .max(ui.spacing().interact_size.y);
    egui::ScrollArea::vertical()
        .id_salt("anim_files")
        .max_height(220.0)
        .auto_shrink([false, false])
        .show_rows(ui, row_height, files.len(), |ui, row_range| {
            for i in row_range {
                let stem = &files[i];
                let sel = loaded.as_deref() == Some(stem.as_str());
                if ui
                    .add(egui::Button::selectable(sel, stem).truncate())
                    .clicked()
                    && !sel
                {
                    app.anim_editor.load_char(game_path, stem);
                }
            }
        });

    ui.separator();

    // Animation list for the loaded char.
    ui.heading("Animations");
    let selected_anim = app.anim_editor.selected_anim;
    let anim_names: Vec<String> = app
        .anim_editor
        .char_def
        .as_ref()
        .map(|cd| cd.animations.iter().map(|a| a.name.clone()).collect())
        .unwrap_or_default();

    // Virtualized rows: only the rows visible in the scroll viewport are laid out each frame.
    let row_height = ui
        .text_style_height(&egui::TextStyle::Body)
        .max(ui.spacing().interact_size.y);
    egui::ScrollArea::vertical()
        .id_salt("anim_list")
        .auto_shrink([false, false])
        .show_rows(ui, row_height, anim_names.len(), |ui, row_range| {
            for i in row_range {
                let name = &anim_names[i];
                let label = if name.is_empty() {
                    format!("[{}] (unnamed)", i)
                } else {
                    format!("[{}] {}", i, name)
                };
                if ui
                    .add(egui::Button::selectable(selected_anim == Some(i), label).truncate())
                    .clicked()
                {
                    app.anim_editor.selected_anim = Some(i);
                    app.anim_editor.selected_kf = None;
                    app.anim_editor.selected_part = None;
                    app.anim_editor.playing = false;
                }
            }
        });

    ui.separator();
    ui.horizontal(|ui| {
        if ui
            .button("New")
            .on_hover_text("Add a blank animation")
            .clicked()
        {
            if let Some(i) = app.anim_editor.add_blank_anim() {
                app.anim_editor.selected_anim = Some(i);
                app.anim_editor.selected_kf = None;
            }
        }
        if ui
            .button("Loop Preset")
            .on_hover_text("Add a looping animation stepping through the first frames")
            .clicked()
        {
            if let Some(i) = app.anim_editor.add_loop_preset() {
                app.anim_editor.selected_anim = Some(i);
                app.anim_editor.selected_kf = None;
            }
        }
    });
    ui.horizontal(|ui| {
        let has_sel = selected_anim.is_some();
        if ui
            .add_enabled(has_sel, egui::Button::new("Clone"))
            .on_hover_text("Duplicate the selected animation under a new name")
            .clicked()
        {
            if let Some(ai) = selected_anim {
                if let Some(i) = app.anim_editor.clone_anim(ai) {
                    app.anim_editor.selected_anim = Some(i);
                    app.anim_editor.selected_kf = None;
                }
            }
        }
        if ui
            .add_enabled(has_sel, egui::Button::new("Delete"))
            .clicked()
        {
            if let (Some(cd), Some(ai)) = (app.anim_editor.char_def.as_mut(), selected_anim) {
                if ai < cd.animations.len() {
                    cd.animations.remove(ai);
                    app.anim_editor.selected_anim = None;
                    app.anim_editor.selected_kf = None;
                    app.anim_editor.dirty = true;
                }
            }
        }
    });
}

fn show_inspector(app: &mut ResalinatedApp, ui: &mut Ui, game_path: &Path) {
    // Header: char meta + save.
    if let Some(cd) = app.anim_editor.char_def.as_mut() {
        ui.heading("Character");
        egui::Grid::new("char_meta").num_columns(2).show(ui, |ui| {
            ui.label("path");
            if ui.text_edit_singleline(&mut cd.path).changed() {
                app.anim_editor.dirty = true;
            }
            ui.end_row();
            ui.label("texName");
            if ui.text_edit_singleline(&mut cd.tex_name).changed() {
                app.anim_editor.dirty = true;
            }
            ui.end_row();
            ui.label("specTex");
            if ui.add(egui::DragValue::new(&mut cd.spec_tex)).changed() {
                app.anim_editor.dirty = true;
            }
            ui.end_row();
        });
    }

    ui.horizontal(|ui| {
        let dirty = app.anim_editor.dirty;
        if ui
            .add_enabled(dirty, egui::Button::new("Save .zsx"))
            .on_hover_text("Write to config/amione.SaS2Resalter/Character/data/<name>.zsx")
            .clicked()
        {
            match app.anim_editor.save(game_path) {
                Ok(()) => app.anim_editor.status = Some("Saved .zsx override".to_string()),
                Err(e) => app.anim_editor.status = Some(e),
            }
        }
        if dirty {
            ui.colored_label(Color32::YELLOW, "unsaved");
        }
    });
    if let Some(s) = &app.anim_editor.status {
        ui.colored_label(Color32::LIGHT_BLUE, s);
    }
    ui.separator();

    egui::ScrollArea::vertical()
        .id_salt("anim_inspector_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            show_keyframe_editor(app, ui);
            ui.separator();
            show_part_editor(app, ui);
        });
}

fn show_keyframe_editor(app: &mut ResalinatedApp, ui: &mut Ui) {
    ui.heading("Animation");
    let Some(ai) = app.anim_editor.selected_anim else {
        ui.label("No animation selected.");
        return;
    };

    // Reset only applies to animations that exist in vanilla (not cloned/created ones).
    if app.anim_editor.is_vanilla_anim(ai) {
        if ui
            .button("Reset Animation to Vanilla")
            .on_hover_text("Restore this animation's keyframes from the vanilla character def")
            .clicked()
        {
            if let Err(e) = app.anim_editor.reset_anim_to_vanilla(ai) {
                app.anim_editor.status = Some(e);
            }
        }
    }

    let frame_count = app
        .anim_editor
        .char_def
        .as_ref()
        .map(|cd| cd.frames.len())
        .unwrap_or(0);

    let mut dirty = false;
    let mut invalidate = false;

    if let Some(cd) = app.anim_editor.char_def.as_mut() {
        if let Some(anim) = cd.animations.get_mut(ai) {
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut anim.name);
            });

            let sel_kf = app.anim_editor.selected_kf;
            if let Some(ki) = sel_kf {
                if let Some(kf) = anim.key_frames.get_mut(ki) {
                    egui::Grid::new("kf_fields").num_columns(2).show(ui, |ui| {
                        ui.label("frame ref");
                        let max_ref = frame_count.saturating_sub(1) as i32;
                        if ui
                            .add(egui::DragValue::new(&mut kf.frame_ref).range(0..=max_ref.max(0)))
                            .changed()
                        {
                            dirty = true;
                            invalidate = true;
                        }
                        ui.end_row();
                        ui.label("duration");
                        if ui
                            .add(egui::DragValue::new(&mut kf.duration).range(1..=100_000))
                            .changed()
                        {
                            dirty = true;
                        }
                        ui.end_row();
                        ui.label("lerp");
                        if ui.checkbox(&mut kf.lerp, "").changed() {
                            dirty = true;
                        }
                        ui.end_row();
                    });

                    ui.label("Scripts:");
                    let mut remove_script = None;
                    for (si, s) in kf.scripts.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            if ui.text_edit_singleline(s).changed() {
                                dirty = true;
                            }
                            if ui.small_button("x").clicked() {
                                remove_script = Some(si);
                            }
                        });
                    }
                    if let Some(si) = remove_script {
                        kf.scripts.remove(si);
                        dirty = true;
                    }
                    if kf.scripts.len() < 255 && ui.button("Add script").clicked() {
                        kf.scripts.push(String::new());
                        dirty = true;
                    }
                } else {
                    ui.label("Select a keyframe in the timeline.");
                }
            } else {
                ui.label("Select a keyframe in the timeline.");
            }
        }
    }

    if dirty {
        app.anim_editor.dirty = true;
    }
    if invalidate {
        app.anim_editor.invalidate_preview();
    }
}

fn show_part_editor(app: &mut ResalinatedApp, ui: &mut Ui) {
    ui.heading("Frame Parts");
    let Some(frame_idx) = app.anim_editor.current_frame_index() else {
        ui.label("Select a keyframe whose frame ref is valid.");
        return;
    };
    ui.label(format!("Frame index: {}", frame_idx));

    // Part selector.
    let parts_len = app
        .anim_editor
        .char_def
        .as_ref()
        .and_then(|cd| cd.frames.get(frame_idx))
        .map(|f| f.parts.len())
        .unwrap_or(0);

    let selected_part = app.anim_editor.selected_part;
    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(selected_part.is_none(), "deselect")
            .clicked()
        {
            app.anim_editor.selected_part = None;
        }
        for i in 0..parts_len {
            if ui
                .selectable_label(selected_part == Some(i), format!("{}", i))
                .clicked()
            {
                app.anim_editor.selected_part = Some(i);
            }
        }
    });

    let mut dirty = false;
    let mut invalidate = false;
    let mut remove_part = false;
    let mut add_part = false;
    let mut revert_part = false;

    // Vanilla copy of the selected part, for the resets (cloned so the editor can be borrowed mutably below).
    let vanilla_part = app
        .anim_editor
        .selected_part
        .and_then(|pi| app.anim_editor.vanilla_part_clone(frame_idx, pi));

    if let Some(cd) = app.anim_editor.char_def.as_mut() {
        if let Some(frame) = cd.frames.get_mut(frame_idx) {
            if let Some(pi) = app.anim_editor.selected_part {
                if let Some(part) = frame.parts.get_mut(pi) {
                    if part_fields(ui, part, vanilla_part.as_ref()) {
                        dirty = true;
                        invalidate = true;
                    }
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        if ui.button("Remove part").clicked() {
                            remove_part = true;
                        }
                        let can_revert = vanilla_part.is_some();
                        if ui
                            .add_enabled(can_revert, egui::Button::new("Revert part"))
                            .on_hover_text(
                                "Restore every field of this part from the vanilla file",
                            )
                            .clicked()
                        {
                            revert_part = true;
                        }
                    });
                }
            } else {
                ui.label("Select a part above.");
            }

            ui.add_space(4.0);
            if frame.parts.len() < 32 && ui.button("Add part").clicked() {
                add_part = true;
            }

            if remove_part {
                if let Some(pi) = app.anim_editor.selected_part {
                    if pi < frame.parts.len() {
                        frame.parts.remove(pi);
                        app.anim_editor.selected_part = None;
                        dirty = true;
                        invalidate = true;
                    }
                }
            } else if revert_part {
                if let Some(pi) = app.anim_editor.selected_part {
                    if let Some(v) = vanilla_part.clone() {
                        if pi < frame.parts.len() {
                            frame.parts[pi] = v;
                            dirty = true;
                            invalidate = true;
                        }
                    }
                }
            } else if add_part {
                frame.parts.push(default_part());
                app.anim_editor.selected_part = Some(frame.parts.len() - 1);
                dirty = true;
                invalidate = true;
            }

            // Clamp a stale selection (e.g. after a part removal elsewhere) to a valid index.
            if let Some(pi) = app.anim_editor.selected_part {
                if pi >= frame.parts.len() {
                    app.anim_editor.selected_part = None;
                }
            }
        }
    }

    if dirty {
        app.anim_editor.dirty = true;
    }
    if invalidate {
        app.anim_editor.invalidate_preview();
    }
}

/// One editable part field, used by the per-field reset buttons.
#[derive(Clone, Copy, PartialEq, Eq)]
enum PartField {
    Idx,
    LocX,
    LocY,
    Rotation,
    ScaleX,
    ScaleY,
    Flip,
    Parent,
    ParentOffX,
    ParentOffY,
    ParentRotOff,
}

/// Value of one field, for the "differs from vanilla" check.
fn field_matches(part: &Part, vanilla: Option<&Part>, field: PartField) -> bool {
    let Some(v) = vanilla else {
        return true; // nothing to reset to
    };
    match field {
        PartField::Idx => part.idx == v.idx,
        PartField::LocX => part.location.0 == v.location.0,
        PartField::LocY => part.location.1 == v.location.1,
        PartField::Rotation => part.rotation == v.rotation,
        PartField::ScaleX => part.scaling.0 == v.scaling.0,
        PartField::ScaleY => part.scaling.1 == v.scaling.1,
        PartField::Flip => part.flip == v.flip,
        PartField::Parent => part.parent == v.parent,
        PartField::ParentOffX => part.parent_loc_offset.0 == v.parent_loc_offset.0,
        PartField::ParentOffY => part.parent_loc_offset.1 == v.parent_loc_offset.1,
        PartField::ParentRotOff => part.parent_rotation_offset == v.parent_rotation_offset,
    }
}

/// Copy one field from the vanilla part.
fn reset_field(part: &mut Part, vanilla: &Part, field: PartField) {
    match field {
        PartField::Idx => part.idx = vanilla.idx,
        PartField::LocX => part.location.0 = vanilla.location.0,
        PartField::LocY => part.location.1 = vanilla.location.1,
        PartField::Rotation => part.rotation = vanilla.rotation,
        PartField::ScaleX => part.scaling.0 = vanilla.scaling.0,
        PartField::ScaleY => part.scaling.1 = vanilla.scaling.1,
        PartField::Flip => part.flip = vanilla.flip,
        PartField::Parent => part.parent = vanilla.parent,
        PartField::ParentOffX => part.parent_loc_offset.0 = vanilla.parent_loc_offset.0,
        PartField::ParentOffY => part.parent_loc_offset.1 = vanilla.parent_loc_offset.1,
        PartField::ParentRotOff => part.parent_rotation_offset = vanilla.parent_rotation_offset,
    }
}

/// Numeric editors for one part, each with a reset-to-vanilla button.
/// Returns true if any value changed.
fn part_fields(ui: &mut Ui, part: &mut Part, vanilla: Option<&Part>) -> bool {
    let mut changed = false;

    // Small reset button for one field: enabled only when it differs from vanilla.
    fn reset_button(ui: &mut Ui, part: &Part, vanilla: Option<&Part>, field: PartField) -> bool {
        let differs = !field_matches(part, vanilla, field);
        let response = ui
            .add_enabled(differs, egui::Button::new("Reset").small())
            .on_hover_text("Restore this field from the vanilla file");
        let clicked = response.clicked();
        ui.end_row();
        clicked
    }

    let mut pending: Option<PartField> = None;
    egui::Grid::new("part_fields")
        .num_columns(3)
        .show(ui, |ui| {
            ui.label("tile idx");
            changed |= ui.add(egui::DragValue::new(&mut part.idx)).changed();
            if reset_button(ui, part, vanilla, PartField::Idx) {
                pending = Some(PartField::Idx);
            }
            ui.label("loc x");
            changed |= ui
                .add(egui::DragValue::new(&mut part.location.0).speed(0.5))
                .changed();
            if reset_button(ui, part, vanilla, PartField::LocX) {
                pending = Some(PartField::LocX);
            }
            ui.label("loc y");
            changed |= ui
                .add(egui::DragValue::new(&mut part.location.1).speed(0.5))
                .changed();
            if reset_button(ui, part, vanilla, PartField::LocY) {
                pending = Some(PartField::LocY);
            }
            ui.label("rotation");
            changed |= ui
                .add(egui::DragValue::new(&mut part.rotation).speed(0.01))
                .changed();
            if reset_button(ui, part, vanilla, PartField::Rotation) {
                pending = Some(PartField::Rotation);
            }
            ui.label("scale x");
            changed |= ui
                .add(egui::DragValue::new(&mut part.scaling.0).speed(0.01))
                .changed();
            if reset_button(ui, part, vanilla, PartField::ScaleX) {
                pending = Some(PartField::ScaleX);
            }
            ui.label("scale y");
            changed |= ui
                .add(egui::DragValue::new(&mut part.scaling.1).speed(0.01))
                .changed();
            if reset_button(ui, part, vanilla, PartField::ScaleY) {
                pending = Some(PartField::ScaleY);
            }
            ui.label("flip");
            changed |= ui
                .add(egui::DragValue::new(&mut part.flip).range(0..=1))
                .changed();
            if reset_button(ui, part, vanilla, PartField::Flip) {
                pending = Some(PartField::Flip);
            }
            ui.label("parent");
            changed |= ui
                .add(egui::DragValue::new(&mut part.parent).range(-1..=31))
                .changed();
            if reset_button(ui, part, vanilla, PartField::Parent) {
                pending = Some(PartField::Parent);
            }
            if part.parent > -1 {
                ui.label("parent off x");
                changed |= ui
                    .add(egui::DragValue::new(&mut part.parent_loc_offset.0).speed(0.5))
                    .changed();
                if reset_button(ui, part, vanilla, PartField::ParentOffX) {
                    pending = Some(PartField::ParentOffX);
                }
                ui.label("parent off y");
                changed |= ui
                    .add(egui::DragValue::new(&mut part.parent_loc_offset.1).speed(0.5))
                    .changed();
                if reset_button(ui, part, vanilla, PartField::ParentOffY) {
                    pending = Some(PartField::ParentOffY);
                }
                ui.label("parent rot off");
                changed |= ui
                    .add(egui::DragValue::new(&mut part.parent_rotation_offset).speed(0.01))
                    .changed();
                if reset_button(ui, part, vanilla, PartField::ParentRotOff) {
                    pending = Some(PartField::ParentRotOff);
                }
            }
        });

    if let (Some(field), Some(v)) = (pending, vanilla) {
        reset_field(part, v, field);
        changed = true;
    }
    changed
}

fn show_preview_and_timeline(app: &mut ResalinatedApp, ui: &mut Ui) {
    // Playback controls.
    ui.horizontal(|ui| {
        let playing = app.anim_editor.playing;
        if ui.button(if playing { "Pause" } else { "Play" }).clicked() {
            app.anim_editor.playing = !playing;
        }
        if ui.button("Stop").clicked() {
            app.anim_editor.playing = false;
            app.anim_editor.selected_kf = Some(0);
        }
    });
    ui.separator();

    // Timeline: keyframes of the selected animation.
    if let Some(ai) = app.anim_editor.selected_anim {
        let kfs: Vec<(i32, i32)> = app
            .anim_editor
            .char_def
            .as_ref()
            .and_then(|cd| cd.animations.get(ai))
            .map(|a| {
                a.key_frames
                    .iter()
                    .map(|k| (k.frame_ref, k.duration))
                    .collect()
            })
            .unwrap_or_default();

        ui.horizontal(|ui| {
            ui.label("Timeline:");
            if ui.small_button("+ keyframe").clicked() {
                if let Some(cd) = app.anim_editor.char_def.as_mut() {
                    if let Some(anim) = cd.animations.get_mut(ai) {
                        anim.key_frames.push(KeyFrame {
                            frame_ref: 0,
                            duration: 4,
                            lerp: false,
                            scripts: Vec::new(),
                        });
                        app.anim_editor.selected_kf = Some(anim.key_frames.len() - 1);
                        app.anim_editor.dirty = true;
                    }
                }
            }
            let can_del = app.anim_editor.selected_kf.is_some();
            if ui
                .add_enabled(can_del, egui::Button::new("- keyframe").small())
                .on_hover_text("Remove the selected keyframe")
                .clicked()
            {
                if let (Some(cd), Some(ki)) = (
                    app.anim_editor.char_def.as_mut(),
                    app.anim_editor.selected_kf,
                ) {
                    if let Some(anim) = cd.animations.get_mut(ai) {
                        if ki < anim.key_frames.len() {
                            anim.key_frames.remove(ki);
                            app.anim_editor.selected_kf = None;
                            app.anim_editor.dirty = true;
                        }
                    }
                }
            }
        });

        egui::ScrollArea::horizontal()
            .id_salt("timeline")
            .max_height(56.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (ki, (fref, dur)) in kfs.iter().enumerate() {
                        let sel = app.anim_editor.selected_kf == Some(ki);
                        let label = format!("#{}\nf{} d{}", ki, fref, dur);
                        if ui.selectable_label(sel, label).clicked() {
                            app.anim_editor.selected_kf = Some(ki);
                            app.anim_editor.selected_part = None;
                            app.anim_editor.playing = false;
                        }
                    }
                });
            });
    }

    ui.separator();

    // Preview.
    match app.anim_editor.current_frame_index() {
        Some(fi) => {
            app.anim_editor.ensure_preview(ui.ctx(), fi);
            if let Some((handle, (w, h))) = app.anim_editor.preview() {
                let avail = ui.available_size();
                let scale = (avail.x / w as f32)
                    .min(avail.y / h as f32)
                    .min(4.0)
                    .max(0.1);
                let size = egui::vec2(w as f32 * scale, h as f32 * scale);
                egui::ScrollArea::both().id_salt("preview").show(ui, |ui| {
                    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                    ui.painter().image(
                        handle.id(),
                        rect,
                        Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                        Color32::WHITE,
                    );
                    draw_part_overlay(app, ui, rect, scale);
                });
            } else {
                ui.colored_label(
                    Color32::YELLOW,
                    "Preview unavailable (missing sheet or cell data).",
                );
            }
        }
        None => {
            ui.label("Select a keyframe with a valid frame reference to preview.");
        }
    }
}

/// Draw a highlight box and a center gizmo on the selected part of the preview frame.
fn draw_part_overlay(app: &ResalinatedApp, ui: &mut Ui, rect: Rect, scale: f32) {
    let Some(pi) = app.anim_editor.selected_part else {
        return;
    };
    let Some(r) = app.anim_editor.part_render_info(pi) else {
        return;
    };
    let painter = ui.painter();

    let to_screen = |x: f32, y: f32| Pos2::new(rect.min.x + x * scale, rect.min.y + y * scale);

    let box_rect = Rect::from_min_max(to_screen(r.min_x, r.min_y), to_screen(r.max_x, r.max_y));
    painter.rect_filled(
        box_rect,
        0.0,
        Color32::from_rgba_unmultiplied(60, 200, 255, 30),
    );
    painter.rect_stroke(
        box_rect,
        0.0,
        Stroke::new(1.5_f32, Color32::from_rgb(60, 200, 255)),
        egui::StrokeKind::Middle,
    );

    // Gizmo: crosshair at the part's center.
    let c = to_screen(r.center_x, r.center_y);
    let arm = 6.0_f32.max(4.0 * scale);
    let gc = Color32::from_rgb(255, 220, 80);
    painter.line_segment(
        [Pos2::new(c.x - arm, c.y), Pos2::new(c.x + arm, c.y)],
        Stroke::new(1.5_f32, gc),
    );
    painter.line_segment(
        [Pos2::new(c.x, c.y - arm), Pos2::new(c.x, c.y + arm)],
        Stroke::new(1.5_f32, gc),
    );
    painter.circle_stroke(c, 3.0_f32.max(2.0 * scale), Stroke::new(1.5_f32, gc));
}

fn default_part() -> Part {
    Part {
        idx: 0,
        location: (0.0, 0.0),
        rotation: 0.0,
        scaling: (1.0, 1.0),
        flip: 0,
        parent: -1,
        parent_loc_offset: (0.0, 0.0),
        parent_rotation_offset: 0.0,
    }
}
