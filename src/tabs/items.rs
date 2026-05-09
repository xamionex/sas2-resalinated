use std::collections::HashMap;
use crate::app::ResalinatedApp;
use crate::atlas::ItemAtlas;
use eframe::egui;
use egui::{Ui, Color32, Response};
use sas2_parser::loot_catalog::{LootDef, LootFieldValue};
use sas2_parser::loot_names;

/// Draw one icon button from the atlas.
/// If either the atlas or the def is missing (or the def has no icon), an invisible placeholder of the same size is rendered so the grid columns stay aligned.
pub fn draw_image_button(
    ui: &mut Ui,
    atlas: Option<&ItemAtlas>,
    def: Option<&LootDef>,
    icon_size: f32,
) -> Response {
    let uv = atlas.zip(def).and_then(|(a, d)| a.icon_uv(d));

    if let (Some(uv), Some(atlas)) = (uv, atlas) {
        ui.add(egui::Button::image(
            egui::Image::from_texture(&atlas.texture)
                .fit_to_exact_size(egui::vec2(icon_size, icon_size))
                .uv(uv),
        ))
    } else {
        ui.allocate_response(egui::vec2(icon_size, icon_size), egui::Sense::click())
    }
}

/// Render a word‑wrapped item name at `font_size` points.
/// Each whitespace‑separated word gets its own truncating label so long names don't overflow their icon column.
pub fn add_item_label(ui: &mut Ui, title: &str, font_size: f32) {
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
    if app.working_catalog.is_none() {
        ui.label("No catalog loaded.");
        return;
    }

    let full_width = ui.available_width();
    let min_size = 250.0;
    let panel_width = if app.config.items_details_panel_width > 0.0 {
        app.config.items_details_panel_width.max(min_size)
    } else {
        full_width * 0.5
    };

    // Right panel: item editor
    let right_panel = egui::Panel::right("item_details")
        .resizable(true)
        .default_size(panel_width)
        .min_size(min_size)
        .max_size(full_width * 0.8)
        .size_range(min_size..=full_width * 0.8)
        .show_inside(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if let Some(idx) = app.selected_item_idx {
                let vanilla_def = app.vanilla_catalog.as_ref().and_then(|vc| {
                    app.working_catalog
                        .as_ref()
                        .unwrap()
                        .loot_defs
                        .get(idx)
                        .and_then(|def| vc.by_name.get(&def.name).map(|&i| &vc.loot_defs[i]))
                }).cloned();
                if let Some(def) = app.working_catalog
                    .as_mut()
                    .unwrap()
                    .loot_defs
                    .get_mut(idx)
                {
                    show_lootdef_editor(ui, def, vanilla_def.as_ref());
                } else {
                    ui.label("Invalid selection.");
                }
            } else {
                ui.label("Select an item to edit.");
            }
        });

    let actual_width = right_panel.response.rect.width();
    if (actual_width - app.config.items_details_panel_width).abs() > 0.1 {
        app.config.items_details_panel_width = actual_width;
        app.config.save();
    }

    // Central panel: search + list (full copy from save editor)
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.set_min_width(200.0);

        // Search & checkbox
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut app.search_filter);
        });
        ui.checkbox(&mut app.show_only_changed_items, "Show Only Changed Items");
        ui.add_space(4.0);

        let filter = app.search_filter.to_lowercase();
        let vanilla = app.vanilla_catalog.as_ref();

        // Build filtered list
        let filtered_indices: Vec<(usize, &LootDef)> = app.working_catalog
            .as_ref()
            .unwrap()
            .loot_defs
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                let matches_search = d.name.to_lowercase().contains(&filter)
                    || d.title.first().map(|t| t.to_lowercase().contains(&filter)).unwrap_or(false);
                if !matches_search { return false; }
                if app.show_only_changed_items {
                    if let Some(vanilla) = vanilla {
                        if let Some(&vi) = vanilla.by_name.get(&d.name) {
                            let vdef = &vanilla.loot_defs[vi];
                            d.to_bytes().ok() != vdef.to_bytes().ok()
                        } else {
                            true
                        }
                    } else {
                        true
                    }
                } else {
                    true
                }
            })
            .collect();

        // Group by type‑subtype category
        let mut grouped: HashMap<String, Vec<(usize, &LootDef)>> = HashMap::new();
        for (idx, def) in filtered_indices {
            let cat = format!(
                "{} - {}",
                loot_names::get_type_name(def.type_),
                loot_names::get_subtype_name(def.type_, def.sub_type)
            );
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
                                let response = draw_image_button(
                                    ui,
                                    app.item_atlas.as_ref(),
                                    Some(def),
                                    app.config.item_icon_size,
                                );
                                let btn_w = response.rect.width();

                                if response.clicked() {
                                    app.selected_item_idx = Some(*orig_idx);
                                }

                                ui.set_max_width(btn_w);
                                let display_name = def.title.first()
                                    .filter(|t| !t.is_empty())
                                    .cloned()
                                    .unwrap_or_else(|| def.name.clone());
                                add_item_label(ui, &display_name, app.config.item_font_size);
                            });
                        }
                    });

                    ui.add_space(8.0);
                }
            });
    });
}

const CHANGED_COLOR: Color32 = Color32::from_rgb(200, 160, 0); // muted gold

fn show_lootdef_editor(ui: &mut Ui, def: &mut LootDef, vanilla: Option<&LootDef>) {
    ui.heading("Loot Definition");

    if let Some(vanilla_def) = vanilla {
        if ui.button("Reset Item to Vanilla").clicked() {
            *def = vanilla_def.clone();
            return;
        }
    }

    ui.separator();

    // Name
    let name_changed = vanilla.map(|v| def.name != v.name).unwrap_or(true);
    ui.horizontal(|ui| {
        let label = if name_changed {
            egui::RichText::new("Name:").color(CHANGED_COLOR)
        } else {
            egui::RichText::new("Name:")
        };
        ui.label(label);
        ui.text_edit_singleline(&mut def.name);
        if let Some(v) = vanilla {
            if def.name != v.name {
                if ui.button("↺").clicked() { def.name = v.name.clone(); }
            }
        }
    });

    // Title
    let title_changed = vanilla.map(|v| def.title[0] != v.title[0]).unwrap_or(true);
    ui.horizontal(|ui| {
        let label = if title_changed {
            egui::RichText::new("Title:").color(CHANGED_COLOR)
        } else {
            egui::RichText::new("Title:")
        };
        ui.label(label);
        ui.text_edit_singleline(&mut def.title[0]);
        if let Some(v) = vanilla {
            if def.title[0] != v.title[0] {
                if ui.button("↺").clicked() { def.title[0] = v.title[0].clone(); }
            }
        }
    });

    // Description
    let desc_changed = vanilla.map(|v| def.description[0] != v.description[0]).unwrap_or(true);
    ui.horizontal(|ui| {
        let label = if desc_changed {
            egui::RichText::new("Description:").color(CHANGED_COLOR)
        } else {
            egui::RichText::new("Description:")
        };
        ui.label(label);
        ui.text_edit_singleline(&mut def.description[0]);
        if let Some(v) = vanilla {
            if def.description[0] != v.description[0] {
                if ui.button("↺").clicked() { def.description[0] = v.description[0].clone(); }
            }
        }
    });

    let type_name = loot_names::get_type_name(def.type_);
    let subtype_name = loot_names::get_subtype_name(def.type_, def.sub_type);
    ui.label(format!("Type: {} ({}) | Subtype: {} ({})", def.type_, type_name, def.sub_type, subtype_name));

    // Cost
    let cost_changed = vanilla.map(|v| (def.cost - v.cost).abs() > 0.001).unwrap_or(true);
    ui.horizontal(|ui| {
        let label = if cost_changed {
            egui::RichText::new("Cost:").color(CHANGED_COLOR)
        } else {
            egui::RichText::new("Cost:")
        };
        ui.label(label);
        let mut cost = def.cost;
        if ui.add(egui::DragValue::new(&mut cost).speed(1.0)).changed() {
            def.cost = cost;
        }
        if let Some(v) = vanilla {
            if (cost - v.cost).abs() > 0.001 {
                if ui.button("↺").clicked() { def.cost = v.cost; }
            }
        }
    });

    // Img
    let img_changed = vanilla.map(|v| def.img != v.img).unwrap_or(true);
    ui.horizontal(|ui| {
        let label = if img_changed {
            egui::RichText::new("Img:").color(CHANGED_COLOR)
        } else {
            egui::RichText::new("Img:")
        };
        ui.label(label);
        ui.add(egui::DragValue::new(&mut def.img));
        if let Some(v) = vanilla {
            if def.img != v.img {
                if ui.button("↺").clicked() { def.img = v.img; }
            }
        }
    });

    // AltImg
    let altimg_changed = vanilla.map(|v| def.alt_img != v.alt_img).unwrap_or(true);
    ui.horizontal(|ui| {
        let label = if altimg_changed {
            egui::RichText::new("AltImg:").color(CHANGED_COLOR)
        } else {
            egui::RichText::new("AltImg:")
        };
        ui.label(label);
        ui.add(egui::DragValue::new(&mut def.alt_img));
        if let Some(v) = vanilla {
            if def.alt_img != v.alt_img {
                if ui.button("↺").clicked() { def.alt_img = v.alt_img; }
            }
        }
    });

    // Texture
    let tex_changed = vanilla.map(|v| def.texture != v.texture).unwrap_or(true);
    ui.horizontal(|ui| {
        let label = if tex_changed {
            egui::RichText::new("Texture:").color(CHANGED_COLOR)
        } else {
            egui::RichText::new("Texture:")
        };
        ui.label(label);
        ui.text_edit_singleline(&mut def.texture);
        if let Some(v) = vanilla {
            if def.texture != v.texture {
                if ui.button("↺").clicked() { def.texture = v.texture.clone(); }
            }
        }
    });

    // Fields, scrollable when there are many
    ui.collapsing(format!("Fields ({})", def.fields.len()), |ui| {
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                for (_i, field) in def.fields.iter_mut().enumerate() {
                    let field_name = loot_names::get_field_name(def.type_, field.id);
                    let changed = vanilla.and_then(|vdef| vdef.fields.iter().find(|vf| vf.id == field.id))
                        .map(|vf| {
                            match (&field.value, &vf.value) {
                                (LootFieldValue::Float(a), LootFieldValue::Float(b)) => (a - b).abs() > 0.001,
                                (LootFieldValue::Int(a), LootFieldValue::Int(b)) => a != b,
                                (LootFieldValue::Bool(a), LootFieldValue::Bool(b)) => a != b,
                                (LootFieldValue::String(a), LootFieldValue::String(b)) => a != b,
                                _ => true,
                            }
                        })
                        .unwrap_or(true);

                    let label = if changed {
                        egui::RichText::new(format!("{}: ", field_name)).color(CHANGED_COLOR)
                    } else {
                        egui::RichText::new(format!("{}: ", field_name))
                    };
                    ui.horizontal(|ui| {
                        ui.label(label);
                        match &mut field.value {
                            LootFieldValue::Float(v) => { ui.add(egui::DragValue::new(v).speed(0.1)); }
                            LootFieldValue::Int(v) => { ui.add(egui::DragValue::new(v)); }
                            LootFieldValue::Bool(v) => { ui.checkbox(v, ""); }
                            LootFieldValue::String(v) => { ui.text_edit_singleline(v); }
                        }
                    });
                }
            });
    });

    // Flags, also scrollable
    ui.collapsing(format!("Flags ({})", def.flags.len()), |ui| {
        egui::ScrollArea::vertical()
            .max_height(200.0)
            .show(ui, |ui| {
                for (_i, flag) in def.flags.iter_mut().enumerate() {
                    let flag_name = loot_names::get_flag_name(def.type_, *flag);
                    ui.horizontal(|ui| {
                        ui.label(format!("Flag {}: {}", flag, flag_name));
                    });
                }
            });
    });
}
