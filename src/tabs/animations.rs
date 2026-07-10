use crate::app::ResalinatedApp;
use eframe::egui;
use egui::{Color32, Ui};
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
        .show_inside(ui, |ui| {
            show_left(app, ui, &game_path);
        });

    if app.anim_editor.char_def.is_none() {
        egui::CentralPanel::default().show_inside(ui, |ui| {
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
        .show_inside(ui, |ui| {
            show_inspector(app, ui, &game_path);
        });

    egui::CentralPanel::default().show_inside(ui, |ui| {
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
    egui::ScrollArea::vertical()
        .id_salt("anim_files")
        .max_height(220.0)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for stem in files {
                let sel = loaded.as_deref() == Some(stem.as_str());
                if ui.selectable_label(sel, &stem).clicked() && !sel {
                    app.anim_editor.load_char(game_path, &stem);
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

    egui::ScrollArea::vertical()
        .id_salt("anim_list")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (i, name) in anim_names.iter().enumerate() {
                let label = if name.is_empty() {
                    format!("[{}] (unnamed)", i)
                } else {
                    format!("[{}] {}", i, name)
                };
                if ui.selectable_label(selected_anim == Some(i), label).clicked() {
                    app.anim_editor.selected_anim = Some(i);
                    app.anim_editor.selected_kf = None;
                    app.anim_editor.selected_part = None;
                    app.anim_editor.playing = false;
                }
            }
        });

    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("New").on_hover_text("Add a blank animation").clicked() {
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

    if let Some(cd) = app.anim_editor.char_def.as_mut() {
        if let Some(frame) = cd.frames.get_mut(frame_idx) {
            if let Some(pi) = app.anim_editor.selected_part {
                if let Some(part) = frame.parts.get_mut(pi) {
                    if part_fields(ui, part) {
                        dirty = true;
                        invalidate = true;
                    }
                    ui.add_space(4.0);
                    if ui.button("Remove part").clicked() {
                        remove_part = true;
                    }
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
            } else if add_part {
                frame.parts.push(default_part());
                app.anim_editor.selected_part = Some(frame.parts.len() - 1);
                dirty = true;
                invalidate = true;
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

/// Numeric editors for one part. Returns true if any value changed.
fn part_fields(ui: &mut Ui, part: &mut Part) -> bool {
    let mut changed = false;
    egui::Grid::new("part_fields").num_columns(2).show(ui, |ui| {
        ui.label("tile idx");
        changed |= ui.add(egui::DragValue::new(&mut part.idx)).changed();
        ui.end_row();
        ui.label("loc x");
        changed |= ui
            .add(egui::DragValue::new(&mut part.location.0).speed(0.5))
            .changed();
        ui.end_row();
        ui.label("loc y");
        changed |= ui
            .add(egui::DragValue::new(&mut part.location.1).speed(0.5))
            .changed();
        ui.end_row();
        ui.label("rotation");
        changed |= ui
            .add(egui::DragValue::new(&mut part.rotation).speed(0.01))
            .changed();
        ui.end_row();
        ui.label("scale x");
        changed |= ui
            .add(egui::DragValue::new(&mut part.scaling.0).speed(0.01))
            .changed();
        ui.end_row();
        ui.label("scale y");
        changed |= ui
            .add(egui::DragValue::new(&mut part.scaling.1).speed(0.01))
            .changed();
        ui.end_row();
        ui.label("flip");
        changed |= ui.add(egui::DragValue::new(&mut part.flip).range(0..=1)).changed();
        ui.end_row();
        ui.label("parent");
        changed |= ui
            .add(egui::DragValue::new(&mut part.parent).range(-1..=31))
            .changed();
        ui.end_row();
        if part.parent > -1 {
            ui.label("parent off x");
            changed |= ui
                .add(egui::DragValue::new(&mut part.parent_loc_offset.0).speed(0.5))
                .changed();
            ui.end_row();
            ui.label("parent off y");
            changed |= ui
                .add(egui::DragValue::new(&mut part.parent_loc_offset.1).speed(0.5))
                .changed();
            ui.end_row();
            ui.label("parent rot off");
            changed |= ui
                .add(egui::DragValue::new(&mut part.parent_rotation_offset).speed(0.01))
                .changed();
            ui.end_row();
        }
    });
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
            .map(|a| a.key_frames.iter().map(|k| (k.frame_ref, k.duration)).collect())
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
            if ui.add_enabled(can_del, egui::Button::new("- keyframe").small()).clicked() {
                if let (Some(cd), Some(ki)) =
                    (app.anim_editor.char_def.as_mut(), app.anim_editor.selected_kf)
                {
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
                    ui.add(egui::Image::from_texture(handle).fit_to_exact_size(size));
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
