use crate::app::ResalinatedApp;
use eframe::egui;
use egui::{Color32, Ui};
use sas2_parser::monster_catalog::{MonsterDef, MonsterFieldValue};
use std::collections::HashMap;

fn add_monster_label(ui: &mut Ui, title: &str, font_size: f32) {
    for word in title.split_whitespace() {
        ui.add(
            egui::Label::new(egui::RichText::new(word).size(font_size))
                .wrap_mode(egui::TextWrapMode::Truncate)
                .halign(egui::Align::Center)
                .show_tooltip_when_elided(false),
        );
    }
}

pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    if app.working_monster_catalog.is_none() {
        ui.label("No monster catalog loaded.");
        return;
    }

    let full_width = ui.available_width();
    let min_size = 250.0;
    let panel_width = if app.config.monsters_details_panel_width > 0.0 {
        app.config.monsters_details_panel_width.max(min_size)
    } else {
        full_width * 0.5
    };

    // Right panel: editor
    let right_panel = egui::Panel::right("monster_details")
        .resizable(true)
        .default_size(panel_width)
        .min_size(min_size)
        .max_size(full_width * 0.8)
        .size_range(min_size..=full_width * 0.8)
        .show_inside(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if let Some(idx) = app.selected_monster_idx {
                let vanilla_def = app.vanilla_monster_catalog.as_ref().and_then(|vc| {
                    app.working_monster_catalog
                        .as_ref().unwrap().monsters.get(idx)
                        .and_then(|def| vc.by_name.get(&def.name).map(|&i| &vc.monsters[i as usize]))
                }).cloned();
                if let Some(def) = app.working_monster_catalog.as_mut().unwrap().monsters.get_mut(idx) {
                    show_monsterdef_editor(ui, def, vanilla_def.as_ref());
                } else { ui.label("Invalid selection."); }
            } else { ui.label("Select a monster to edit."); }
        });

    let actual_width = right_panel.response.rect.width();
    if (actual_width - app.config.monsters_details_panel_width).abs() > 0.1 {
        app.config.monsters_details_panel_width = actual_width;
        app.config.save();
    }

    // Central panel: search + list with icons
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.set_min_width(200.0);

        // search & checkbox
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut app.monster_search_filter);
        });
        ui.checkbox(&mut app.show_only_changed_monsters, "Show Only Changed Monsters");
        ui.add_space(4.0);

        let filter = app.monster_search_filter.to_lowercase();
        let vanilla = app.vanilla_monster_catalog.as_ref();

        // Build filtered list
        let filtered: Vec<(usize, &MonsterDef)> = app.working_monster_catalog
            .as_ref().unwrap().monsters.iter().enumerate()
            .filter(|(_, d)| {
                let matches = d.name.to_lowercase().contains(&filter)
                    || d.titles.first().map(|t| t.to_lowercase().contains(&filter)).unwrap_or(false);
                if !matches { return false; }
                if app.show_only_changed_monsters {
                    if let Some(vanilla) = vanilla {
                        if let Some(&vi) = vanilla.by_name.get(&d.name) {
                            let vdef = &vanilla.monsters[vi as usize];
                            d.to_bytes().ok() != vdef.to_bytes().ok()
                        } else { true }
                    } else { true }
                } else { true }
            })
            .collect();

        // Group by type‑subtype
        let mut grouped: HashMap<String, Vec<(usize, &MonsterDef)>> = HashMap::new();
        for (idx, def) in filtered {
            let cat = format!("Type {} - SubType {}", def.type_, def.sub_type);
            grouped.entry(cat).or_default().push((idx, def));
        }
        let mut categories: Vec<_> = grouped.keys().cloned().collect();
        categories.sort();

        egui::ScrollArea::both()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                for cat in categories {
                    let entries = grouped.get(&cat).unwrap();
                    ui.style_mut().interaction.selectable_labels = false;
                    ui.label(egui::RichText::new(&cat).strong());

                    egui::Grid::new(&cat).spacing([8.0, 8.0]).show(ui, |ui| {
                        for (orig_idx, def) in entries {
                            ui.vertical(|ui| {
                                let tex = if def.texture.is_empty() {
                                    None
                                } else {
                                    app.monster_texture_cache.get_or_load(&def.texture)
                                };
                                let response = if let Some(tex) = &tex {
                                    // Calculate UV for the first 128x128 frame
                                    let size = tex.size_vec2();          // TextureHandle knows its pixel size
                                    let uv = crate::atlas::monster_idle_uv(size[0] as u32, size[1] as u32);
                                    ui.add(egui::Button::image(
                                        egui::Image::from_texture(tex)
                                            .fit_to_exact_size(egui::vec2(app.config.item_icon_size, app.config.item_icon_size))
                                            .uv(uv),
                                    ))
                                } else {
                                    // placeholder while loading
                                    ui.allocate_response(egui::vec2(app.config.item_icon_size, app.config.item_icon_size), egui::Sense::click())
                                };
                                let btn_w = response.rect.width();
                                if response.clicked() {
                                    app.selected_monster_idx = Some(*orig_idx);
                                }
                                ui.set_max_width(btn_w);
                                let display_name = def.titles.first()
                                    .filter(|t| !t.is_empty())
                                    .cloned()
                                    .unwrap_or_else(|| def.name.clone());
                                add_monster_label(ui, &display_name, app.config.item_font_size);
                            });
                        }
                    });

                    ui.add_space(8.0);
                }
            });
    });
}

const CHANGED_COLOR: Color32 = Color32::from_rgb(200, 160, 0);

fn show_monsterdef_editor(ui: &mut Ui, def: &mut MonsterDef, vanilla: Option<&MonsterDef>) {
    ui.heading("Monster Definition");
    if let Some(vanilla_def) = vanilla {
        if ui.button("Reset Monster to Vanilla").clicked() { *def = vanilla_def.clone(); return; }
    }
    ui.separator();

    // Helper to draw a label that changes color when the value differs
    let colored = |ui: &mut Ui, label: &str, changed: bool| {
        if changed { ui.colored_label(CHANGED_COLOR, label); } else { ui.label(label); }
    };

    // Name
    let name_changed = vanilla.map(|v| def.name != v.name).unwrap_or(true);
    ui.horizontal(|ui| {
        colored(ui, "Name:", name_changed);
        ui.text_edit_singleline(&mut def.name);
        if let Some(v) = vanilla { if def.name != v.name { if ui.button("↺").clicked() { def.name = v.name.clone(); } } }
    });

    // Title (first entry)
    let title_changed = vanilla.map(|v| def.titles[0] != v.titles[0]).unwrap_or(true);
    ui.horizontal(|ui| {
        colored(ui, "Title:", title_changed);
        ui.text_edit_singleline(&mut def.titles[0]);
        if let Some(v) = vanilla { if def.titles[0] != v.titles[0] { if ui.button("↺").clicked() { def.titles[0] = v.titles[0].clone(); } } }
    });

    // Description (first entry)
    let desc_changed = vanilla.map(|v| def.descriptions[0] != v.descriptions[0]).unwrap_or(true);
    ui.horizontal(|ui| {
        colored(ui, "Description:", desc_changed);
        ui.text_edit_singleline(&mut def.descriptions[0]);
        if let Some(v) = vanilla { if def.descriptions[0] != v.descriptions[0] { if ui.button("↺").clicked() { def.descriptions[0] = v.descriptions[0].clone(); } } }
    });

    // Type & SubType
    let type_changed = vanilla.map(|v| def.type_ != v.type_).unwrap_or(false);
    let subtype_changed = vanilla.map(|v| def.sub_type != v.sub_type).unwrap_or(false);
    ui.horizontal(|ui| {
        colored(ui, "Type:", type_changed);
        ui.add(egui::DragValue::new(&mut def.type_));
        if let Some(v) = vanilla { if def.type_ != v.type_ { if ui.button("↺").clicked() { def.type_ = v.type_; } } }
        ui.label("SubType:");
        if subtype_changed { ui.colored_label(CHANGED_COLOR, ""); }
        ui.add(egui::DragValue::new(&mut def.sub_type));
        if let Some(v) = vanilla { if def.sub_type != v.sub_type { if ui.button("↺").clicked() { def.sub_type = v.sub_type; } } }
    });

    // Cost
    let cost_changed = vanilla.map(|v| (def.cost - v.cost).abs() > 0.001).unwrap_or(true);
    ui.horizontal(|ui| {
        colored(ui, "Cost:", cost_changed);
        let mut cost = def.cost;
        if ui.add(egui::DragValue::new(&mut cost).speed(1.0)).changed() { def.cost = cost; }
        if let Some(v) = vanilla { if (cost - v.cost).abs() > 0.001 { if ui.button("↺").clicked() { def.cost = v.cost; } } }
    });

    // Img
    let img_changed = vanilla.map(|v| def.img != v.img).unwrap_or(true);
    ui.horizontal(|ui| {
        colored(ui, "Img:", img_changed);
        ui.add(egui::DragValue::new(&mut def.img));
        if let Some(v) = vanilla { if def.img != v.img { if ui.button("↺").clicked() { def.img = v.img; } } }
    });

    // AltImg
    let altimg_changed = vanilla.map(|v| def.alt_img != v.alt_img).unwrap_or(true);
    ui.horizontal(|ui| {
        colored(ui, "AltImg:", altimg_changed);
        ui.add(egui::DragValue::new(&mut def.alt_img));
        if let Some(v) = vanilla { if def.alt_img != v.alt_img { if ui.button("↺").clicked() { def.alt_img = v.alt_img; } } }
    });

    // Texture
    let tex_changed = vanilla.map(|v| def.texture != v.texture).unwrap_or(true);
    ui.horizontal(|ui| {
        colored(ui, "Texture:", tex_changed);
        ui.text_edit_singleline(&mut def.texture);
        if let Some(v) = vanilla { if def.texture != v.texture { if ui.button("↺").clicked() { def.texture = v.texture.clone(); } } }
    });

    // Def
    let def_changed = vanilla.map(|v| def.def != v.def).unwrap_or(true);
    ui.horizontal(|ui| {
        colored(ui, "Def:", def_changed);
        ui.text_edit_singleline(&mut def.def);
        if let Some(v) = vanilla { if def.def != v.def { if ui.button("↺").clicked() { def.def = v.def.clone(); } } }
    });

    // Box / shadow dimensions
    {
        let changed = vanilla.map(|v| def.box_width != v.box_width).unwrap_or(true);
        ui.horizontal(|ui| {
            colored(ui, "Box Width:", changed);
            ui.add(egui::DragValue::new(&mut def.box_width));
            if let Some(v) = vanilla { if def.box_width != v.box_width { if ui.button("↺").clicked() { def.box_width = v.box_width; } } }
        });
    }
    {
        let changed = vanilla.map(|v| def.box_height != v.box_height).unwrap_or(true);
        ui.horizontal(|ui| {
            colored(ui, "Box Height:", changed);
            ui.add(egui::DragValue::new(&mut def.box_height));
            if let Some(v) = vanilla { if def.box_height != v.box_height { if ui.button("↺").clicked() { def.box_height = v.box_height; } } }
        });
    }
    {
        let changed = vanilla.map(|v| def.box_sub_height != v.box_sub_height).unwrap_or(true);
        ui.horizontal(|ui| {
            colored(ui, "Box Sub Height:", changed);
            ui.add(egui::DragValue::new(&mut def.box_sub_height));
            if let Some(v) = vanilla { if def.box_sub_height != v.box_sub_height { if ui.button("↺").clicked() { def.box_sub_height = v.box_sub_height; } } }
        });
    }
    {
        let changed = vanilla.map(|v| def.shadow_width != v.shadow_width).unwrap_or(true);
        ui.horizontal(|ui| {
            colored(ui, "Shadow Width:", changed);
            ui.add(egui::DragValue::new(&mut def.shadow_width));
            if let Some(v) = vanilla { if def.shadow_width != v.shadow_width { if ui.button("↺").clicked() { def.shadow_width = v.shadow_width; } } }
        });
    }
    {
        let changed = vanilla.map(|v| def.shadow_height != v.shadow_height).unwrap_or(true);
        ui.horizontal(|ui| {
            colored(ui, "Shadow Height:", changed);
            ui.add(egui::DragValue::new(&mut def.shadow_height));
            if let Some(v) = vanilla { if def.shadow_height != v.shadow_height { if ui.button("↺").clicked() { def.shadow_height = v.shadow_height; } } }
        });
    }

    // Fields, scrollable
    ui.collapsing(format!("Fields ({})", def.fields.len()), |ui| {
        egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
            for field in def.fields.iter_mut() {
                let fname = format!("Field {}", field.id);
                let changed = vanilla.and_then(|vdef| vdef.fields.iter().find(|vf| vf.id == field.id))
                    .map(|vf| match (&field.value, &vf.value) {
                        (MonsterFieldValue::Float(a), MonsterFieldValue::Float(b)) => (a - b).abs() > 0.001,
                        (MonsterFieldValue::Int(a),   MonsterFieldValue::Int(b))   => a != b,
                        (MonsterFieldValue::String(a),MonsterFieldValue::String(b))=> a != b,
                        _ => true,
                    }).unwrap_or(true);
                let label = if changed { egui::RichText::new(fname).color(CHANGED_COLOR) } else { egui::RichText::new(fname) };
                ui.horizontal(|ui| {
                    ui.label(label);
                    match &mut field.value {
                        MonsterFieldValue::Float(v) => { ui.add(egui::DragValue::new(v).speed(0.1)); }
                        MonsterFieldValue::Int(v)   => { ui.add(egui::DragValue::new(v)); }
                        MonsterFieldValue::String(v)=> { ui.text_edit_singleline(v); }
                    }
                    if let Some(vanilla_def) = vanilla {
                        if let Some(vf) = vanilla_def.fields.iter().find(|vf| vf.id == field.id) {
                            let different = match (&field.value, &vf.value) {
                                (MonsterFieldValue::Float(a), MonsterFieldValue::Float(b)) => (a - b).abs() > 0.001,
                                (MonsterFieldValue::Int(a),   MonsterFieldValue::Int(b))   => a != b,
                                (MonsterFieldValue::String(a),MonsterFieldValue::String(b))=> a != b,
                                _ => true,
                            };
                            if different {
                                if ui.button("↺").clicked() {
                                    field.value = vf.value.clone();
                                }
                            }
                        }
                    }
                });
            }
        });
    });

    // Flags
    ui.collapsing(format!("Flags ({})", def.flags.len()), |ui| {
        for flag in &def.flags { ui.label(format!("Flag {}", flag)); }
    });
}
