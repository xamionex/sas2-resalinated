use crate::app::ResalinatedApp;
use crate::atlas::ItemAtlas;
use crate::magic_slot::MagicSlotOverrides;
use eframe::egui;
use egui::{Color32, Response, Ui};
use sas2_parser::loot_catalog::{LootDef, LootFieldValue};
use sas2_parser::loot_names;
use std::collections::HashMap;

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

/// Render a word-wrapped item name.
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

    // Build magic items list once for the dropdown picker
    let magic_items: Vec<(String, String)> = app.working_catalog
        .as_ref()
        .map(|c| {
            c.loot_defs.iter()
                .filter(|d| d.type_ == 7)
                .map(|d| {
                    let display = d.title.first()
                        .filter(|t| !t.is_empty())
                        .cloned()
                        .unwrap_or_else(|| d.name.clone());
                    (d.name.clone(), display)
                })
                .collect()
        })
        .unwrap_or_default();

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
                    show_lootdef_editor(
                        ui,
                        def,
                        vanilla_def.as_ref(),
                        &magic_items,
                        &mut app.magic_slot_overrides,
                    );
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
        app.config_save_timer = 0.25;
    }

    // Central panel: search + list
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
                        } else { true }
                    } else { true }
                } else { true }
            })
            .collect();

        // Group by type-subtype category
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

const CHANGED_COLOR: Color32 = Color32::from_rgb(200, 160, 0);

/// 'magic_items': (internal_name, display_title) pairs for all magic-type items in the catalog.
fn show_lootdef_editor(
    ui: &mut Ui,
    def: &mut LootDef,
    vanilla: Option<&LootDef>,
    magic_items: &[(String, String)],
    magic_slot_overrides: &mut HashMap<String, HashMap<i32, MagicSlotOverrides>>,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.heading("Loot Definition");

            if let Some(vanilla_def) = vanilla {
                if ui.button("Reset Item to Vanilla").clicked() {
                    *def = vanilla_def.clone();
                    return;
                }
            }

            ui.separator();

            // Scalar fields

            // Name
            field_row(ui, "Name:", vanilla.map(|v| def.name != v.name).unwrap_or(true), |ui| {
                ui.text_edit_singleline(&mut def.name);
                if let Some(v) = vanilla {
                    if def.name != v.name && ui.button("↺").clicked() { def.name = v.name.clone(); }
                }
            });

            // Title
            field_row(ui, "Title:", vanilla.map(|v| def.title[0] != v.title[0]).unwrap_or(true), |ui| {
                ui.text_edit_singleline(&mut def.title[0]);
                if let Some(v) = vanilla {
                    if def.title[0] != v.title[0] && ui.button("↺").clicked() { def.title[0] = v.title[0].clone(); }
                }
            });

            // Description
            field_row(ui, "Description:", vanilla.map(|v| def.description[0] != v.description[0]).unwrap_or(true), |ui| {
                ui.text_edit_singleline(&mut def.description[0]);
                if let Some(v) = vanilla {
                    if def.description[0] != v.description[0] && ui.button("↺").clicked() {
                        def.description[0] = v.description[0].clone();
                    }
                }
            });

            ui.label(format!(
                "Type: {} ({}) | Subtype: {} ({})",
                def.type_,
                loot_names::get_type_name(def.type_),
                def.sub_type,
                loot_names::get_subtype_name(def.type_, def.sub_type)
            ));

            // Cost
            field_row(ui, "Cost:", vanilla.map(|v| (def.cost - v.cost).abs() > 0.001).unwrap_or(true), |ui| {
                let mut cost = def.cost;
                if ui.add(egui::DragValue::new(&mut cost).speed(1.0)).changed() { def.cost = cost; }
                if let Some(v) = vanilla {
                    if (cost - v.cost).abs() > 0.001 && ui.button("↺").clicked() { def.cost = v.cost; }
                }
            });

            // Img
            field_row(ui, "Img:", vanilla.map(|v| def.img != v.img).unwrap_or(true), |ui| {
                ui.add(egui::DragValue::new(&mut def.img));
                if let Some(v) = vanilla {
                    if def.img != v.img && ui.button("↺").clicked() { def.img = v.img; }
                }
            });

            // AltImg
            field_row(ui, "AltImg:", vanilla.map(|v| def.alt_img != v.alt_img).unwrap_or(true), |ui| {
                ui.add(egui::DragValue::new(&mut def.alt_img));
                if let Some(v) = vanilla {
                    if def.alt_img != v.alt_img && ui.button("↺").clicked() { def.alt_img = v.alt_img; }
                }
            });

            // Texture
            field_row(ui, "Texture:", vanilla.map(|v| def.texture != v.texture).unwrap_or(true), |ui| {
                ui.text_edit_singleline(&mut def.texture);
                if let Some(v) = vanilla {
                    if def.texture != v.texture && ui.button("↺").clicked() { def.texture = v.texture.clone(); }
                }
            });

            ui.separator();

            // Numeric / String Fields
            ui.collapsing(format!("Fields ({})", def.fields.len()), |ui| {
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        let weapon_name = def.name.clone();

                        for field in def.fields.iter_mut() {
                            let field_name = loot_names::get_field_name(def.type_, field.id);
                            let changed = vanilla
                                .and_then(|vdef| vdef.fields.iter().find(|vf| vf.id == field.id))
                                .map(|vf| values_differ(&field.value, &vf.value))
                                .unwrap_or(true);

                            let label = if changed {
                                egui::RichText::new(format!("{}: ", field_name)).color(CHANGED_COLOR)
                            } else {
                                egui::RichText::new(format!("{}: ", field_name))
                            };

                            // For weapon magic slots, use a combo box instead of the generic editor.
                            let is_magic_slot = def.type_ == 1 && (field.id == 14 || field.id == 15 || field.id == 16);

                            if is_magic_slot {
                                let current = match &field.value {
                                    LootFieldValue::String(s) => s.clone(),
                                    _ => String::new(),
                                };
                                let vanilla_val = vanilla
                                    .and_then(|v| v.fields.iter().find(|vf| vf.id == field.id))
                                    .map(|vf| match &vf.value { LootFieldValue::String(s) => s.clone(), _ => String::new() });

                                let current_display = if current.is_empty() {
                                    "- None -".to_string()
                                } else {
                                    magic_items.iter()
                                        .find(|(n, _)| n == &current)
                                        .map(|(_, t)| format!("{} ({})", t, current))
                                        .unwrap_or_else(|| format!("? {}", current))
                                };

                                ui.horizontal(|ui| {
                                    ui.label(label);
                                    egui::ComboBox::from_id_salt(format!("magic_slot_{}_{}", def.name, field.id))
                                        .selected_text(&current_display)
                                        .width(260.0)
                                        .show_ui(ui, |ui| {
                                            if ui.selectable_label(current.is_empty(), "- None -").clicked() {
                                                field.value = LootFieldValue::String(String::new());
                                            }
                                            for (name, title) in magic_items {
                                                let entry_label = format!("{} ({})", title, name);
                                                if ui.selectable_label(&current == name, &entry_label).clicked() {
                                                    field.value = LootFieldValue::String(name.clone());
                                                }
                                            }
                                        });
                                    // Vanilla reset
                                    if let Some(van) = &vanilla_val {
                                        if van != &current && ui.button("↺").clicked() {
                                            field.value = LootFieldValue::String(van.clone());
                                        }
                                    }
                                });

                                // Magic damage override after each magic slot field
                                let slot_id = field.id;
                                let weapon_overrides = magic_slot_overrides.entry(weapon_name.clone()).or_default();
                                let slot_override = weapon_overrides.entry(slot_id).or_default();

                                let dmg_label = match slot_id {
                                    14 => "Magic [X] Damage:",
                                    15 => "Magic [Y] Damage:",
                                    16 => "Magic [B] Damage:",
                                    _ => "Magic Damage:",
                                };

                                let changed = (slot_override.damage - 0.0f32).abs() > 0.001;
                                let label_rich = if changed {
                                    egui::RichText::new(format!("{}", dmg_label)).color(CHANGED_COLOR)
                                } else {
                                    egui::RichText::new(format!("{}", dmg_label))
                                };

                                ui.horizontal(|ui| {
                                    ui.label(label_rich);
                                    ui.add(
                                        egui::DragValue::new(&mut slot_override.damage)
                                            .speed(1.0)
                                            .fixed_decimals(0)
                                    );
                                    if changed {
                                        if ui.button("↺").clicked() {
                                            slot_override.damage = 0.0;
                                        }
                                    }
                                });
                            } else {
                            ui.horizontal(|ui| {
                                ui.label(label);
                                    match &mut field.value {
                                        LootFieldValue::Float(v)  => { ui.add(egui::DragValue::new(v).speed(0.1)); }
                                        LootFieldValue::Int(v)    => { ui.add(egui::DragValue::new(v)); }
                                        LootFieldValue::Bool(v)   => { ui.checkbox(v, ""); }
                                        LootFieldValue::String(v) => { ui.text_edit_singleline(v); }
                                    }
                                    // Reset to vanilla
                                    if let Some(vanilla_def) = vanilla {
                                        if let Some(vf) = vanilla_def.fields.iter().find(|vf| vf.id == field.id) {
                                            if values_differ(&field.value, &vf.value)
                                                && ui.button("↺").clicked()
                                            {
                                                field.value = vf.value.clone();
                                            }
                                        }
                                    }
                                });
                            }
                        }
                    });
            });

            // Flags
            let flag_count = loot_names::get_loot_flag_count(def.type_);
            ui.collapsing(
                format!("Flags ({} active / {} total)", def.flags.len(), flag_count),
                |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            show_flag_checkboxes(
                                ui,
                                &mut def.flags,
                                flag_count,
                                def.type_,
                                vanilla.map(|v| &v.flags),
                                loot_names::get_flag_name,
                            );
                        });
                },
            );
        });
}

/// Render the full flag list as checkboxes, one per defined flag index.
/// 'get_name_fn' maps (type_, flag_idx) → &str.
fn show_flag_checkboxes(
    ui: &mut Ui,
    flags: &mut Vec<i32>,
    flag_count: i32,
    type_: i32,
    vanilla_flags: Option<&Vec<i32>>,
    get_name_fn: fn(i32, i32) -> &'static str,
) {
    for i in 0..flag_count {
        let is_set = flags.contains(&i);
        let vanilla_set = vanilla_flags.map(|vf| vf.contains(&i));
        let changed = vanilla_set.map(|vs| vs != is_set).unwrap_or(false);

        let label = format!("[{}] {}", i, get_name_fn(type_, i));
        let label_rich = if changed {
            egui::RichText::new(&label).color(CHANGED_COLOR)
        } else {
            egui::RichText::new(&label)
        };

        let mut checked = is_set;
        if ui.checkbox(&mut checked, label_rich).changed() {
            if checked {
                if !flags.contains(&i) {
                    flags.push(i);
                    flags.sort_unstable();
                }
            } else {
                flags.retain(|&f| f != i);
            }
        }
    }

    // Show any out-of-range flags the item actually has (future-proofing)
    let extra: Vec<i32> = flags.iter().copied().filter(|&f| f >= flag_count).collect();
    if !extra.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new("Unknown flags (raw):").italics());
        for f in extra {
            ui.label(format!("  [{}] Unknown Flag", f));
        }
    }
}

/// Helper to draw a labeled horizontal row with change-tracking color.
fn field_row(
    ui: &mut Ui,
    label: &str,
    changed: bool,
    content: impl FnOnce(&mut Ui),
) {
    ui.horizontal(|ui| {
        if changed {
            ui.colored_label(CHANGED_COLOR, label);
        } else {
            ui.label(label);
        }
        content(ui);
    });
}

fn values_differ(a: &LootFieldValue, b: &LootFieldValue) -> bool {
    match (a, b) {
        (LootFieldValue::Float(x), LootFieldValue::Float(y)) => (x - y).abs() > 0.001,
        (LootFieldValue::Int(x),   LootFieldValue::Int(y))   => x != y,
        (LootFieldValue::Bool(x),  LootFieldValue::Bool(y))  => x != y,
        (LootFieldValue::String(x),LootFieldValue::String(y))=> x != y,
        _ => true,
    }
}