use crate::app::ResalinatedApp;
use crate::tabs::utils::{field_row, CHANGED_COLOR};
use eframe::egui;
use egui::Ui;
use sas2_parser::monster_catalog::{MonsterDef, MonsterFieldValue};
use sas2_parser::monster_names;

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
                let vanilla_def = app
                    .vanilla_monster_catalog
                    .as_ref()
                    .and_then(|vc| {
                        app.working_monster_catalog
                            .as_ref()
                            .unwrap()
                            .monsters
                            .get(idx)
                            .and_then(|def| {
                                vc.by_name.get(&def.name).map(|&i| &vc.monsters[i as usize])
                            })
                    })
                    .cloned();
                if let Some(def) = app
                    .working_monster_catalog
                    .as_mut()
                    .unwrap()
                    .monsters
                    .get_mut(idx)
                {
                    show_monsterdef_editor(ui, def, vanilla_def.as_ref());
                } else {
                    ui.label("Invalid selection.");
                }
            } else {
                ui.label("Select a monster to edit.");
            }
        });

    let actual_width = right_panel.response.rect.width();
    if (actual_width - app.config.monsters_details_panel_width).abs() > 0.1 {
        app.config.monsters_details_panel_width = actual_width;
        app.config_save_timer = 0.25;
    }

    // Central panel: search + list
    egui::CentralPanel::default().show_inside(ui, |ui| {
        ui.set_min_width(200.0);

        // search & checkbox
        ui.horizontal(|ui| {
            ui.label("Search:");
            ui.text_edit_singleline(&mut app.monster_search_filter);
        });
        ui.checkbox(
            &mut app.show_only_changed_monsters,
            "Show Only Changed Monsters",
        );
        ui.add_space(4.0);

        let filter = app.monster_search_filter.to_lowercase();
        let vanilla = app.vanilla_monster_catalog.as_ref();

        // Build filtered list
        let filtered: Vec<(usize, &MonsterDef)> = app
            .working_monster_catalog
            .as_ref()
            .unwrap()
            .monsters
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                let matches = d.name.to_lowercase().contains(&filter)
                    || d.titles
                        .first()
                        .map(|t| t.to_lowercase().contains(&filter))
                        .unwrap_or(false);
                if !matches {
                    return false;
                }
                if app.show_only_changed_monsters {
                    if let Some(vanilla) = vanilla {
                        if let Some(&vi) = vanilla.by_name.get(&d.name) {
                            let vdef = &vanilla.monsters[vi as usize];
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

        let mut grouped: std::collections::HashMap<String, Vec<(usize, &MonsterDef)>> =
            std::collections::HashMap::new();
        for (idx, def) in filtered {
            let cat = format!(
                "{} - SubType {}",
                monster_names::get_monster_type_name(def.type_),
                def.sub_type
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
                                let tex = if def.texture.is_empty() {
                                    None
                                } else {
                                    app.monster_texture_cache.get_or_assemble(
                                        ui.ctx(),
                                        &def.def,
                                        &def.texture,
                                    )
                                };
                                let response = if let Some(tex) = &tex {
                                    ui.add(egui::Button::image(
                                        egui::Image::from_texture(&tex.clone()).fit_to_exact_size(
                                            egui::vec2(
                                                app.config.item_icon_size,
                                                app.config.item_icon_size,
                                            ),
                                        ),
                                    ))
                                } else {
                                    // placeholder while loading
                                    ui.allocate_response(
                                        egui::vec2(
                                            app.config.item_icon_size,
                                            app.config.item_icon_size,
                                        ),
                                        egui::Sense::click(),
                                    )
                                };
                                let btn_w = response.rect.width();
                                if response.clicked() {
                                    app.selected_monster_idx = Some(*orig_idx);
                                }
                                ui.set_max_width(btn_w);
                                let display_name = def
                                    .titles
                                    .first()
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

fn monster_values_differ(a: &MonsterFieldValue, b: &MonsterFieldValue) -> bool {
    match (a, b) {
        (MonsterFieldValue::Float(x), MonsterFieldValue::Float(y)) => (x - y).abs() > 0.001,
        (MonsterFieldValue::Int(x), MonsterFieldValue::Int(y)) => x != y,
        (MonsterFieldValue::String(x), MonsterFieldValue::String(y)) => x != y,
        _ => true,
    }
}

fn show_monsterdef_editor(ui: &mut Ui, def: &mut MonsterDef, vanilla: Option<&MonsterDef>) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.heading("Monster Definition");
            if let Some(vanilla_def) = vanilla {
                if ui.button("Reset Monster to Vanilla").clicked() {
                    *def = vanilla_def.clone();
                    return;
                }
            }
            ui.separator();

            // Name
            field_row(
                ui,
                "Name:",
                vanilla.map(|v| def.name != v.name).unwrap_or(true),
                |ui| {
                    ui.text_edit_singleline(&mut def.name);
                    if let Some(v) = vanilla {
                        if def.name != v.name && ui.button("↺").clicked() {
                            def.name = v.name.clone();
                        }
                    }
                },
            );

            // Title
            field_row(
                ui,
                "Title:",
                vanilla
                    .map(|v| def.titles[0] != v.titles[0])
                    .unwrap_or(true),
                |ui| {
                    ui.text_edit_singleline(&mut def.titles[0]);
                    if let Some(v) = vanilla {
                        if def.titles[0] != v.titles[0] && ui.button("↺").clicked() {
                            def.titles[0] = v.titles[0].clone();
                        }
                    }
                },
            );

            // Description
            field_row(
                ui,
                "Description:",
                vanilla
                    .map(|v| def.descriptions[0] != v.descriptions[0])
                    .unwrap_or(true),
                |ui| {
                    ui.text_edit_singleline(&mut def.descriptions[0]);
                    if let Some(v) = vanilla {
                        if def.descriptions[0] != v.descriptions[0] && ui.button("↺").clicked() {
                            def.descriptions[0] = v.descriptions[0].clone();
                        }
                    }
                },
            );

            // Type / SubType
            ui.horizontal(|ui| {
                let tc = vanilla.map(|v| def.type_ != v.type_).unwrap_or(false);
                let sc = vanilla.map(|v| def.sub_type != v.sub_type).unwrap_or(false);
                if tc {
                    ui.colored_label(CHANGED_COLOR, "Type:");
                } else {
                    ui.label("Type:");
                }
                ui.add(egui::DragValue::new(&mut def.type_));
                if let Some(v) = vanilla {
                    if def.type_ != v.type_ && ui.button("↺").clicked() {
                        def.type_ = v.type_;
                    }
                }
                if sc {
                    ui.colored_label(CHANGED_COLOR, "SubType:");
                } else {
                    ui.label("SubType:");
                }
                ui.add(egui::DragValue::new(&mut def.sub_type));
                if let Some(v) = vanilla {
                    if def.sub_type != v.sub_type && ui.button("↺").clicked() {
                        def.sub_type = v.sub_type;
                    }
                }
                ui.label(format!(
                    "({})",
                    monster_names::get_monster_type_name(def.type_)
                ));
            });

            // Cost / Img / AltImg
            field_row(
                ui,
                "Cost:",
                vanilla
                    .map(|v| (def.cost - v.cost).abs() > 0.001)
                    .unwrap_or(true),
                |ui| {
                    let mut cost = def.cost;
                    if ui.add(egui::DragValue::new(&mut cost).speed(1.0)).changed() {
                        def.cost = cost;
                    }
                    if let Some(v) = vanilla {
                        if (cost - v.cost).abs() > 0.001 && ui.button("↺").clicked() {
                            def.cost = v.cost;
                        }
                    }
                },
            );
            field_row(
                ui,
                "Img:",
                vanilla.map(|v| def.img != v.img).unwrap_or(true),
                |ui| {
                    ui.add(egui::DragValue::new(&mut def.img));
                    if let Some(v) = vanilla {
                        if def.img != v.img && ui.button("↺").clicked() {
                            def.img = v.img;
                        }
                    }
                },
            );
            field_row(
                ui,
                "AltImg:",
                vanilla.map(|v| def.alt_img != v.alt_img).unwrap_or(true),
                |ui| {
                    ui.add(egui::DragValue::new(&mut def.alt_img));
                    if let Some(v) = vanilla {
                        if def.alt_img != v.alt_img && ui.button("↺").clicked() {
                            def.alt_img = v.alt_img;
                        }
                    }
                },
            );
            field_row(
                ui,
                "Texture:",
                vanilla.map(|v| def.texture != v.texture).unwrap_or(true),
                |ui| {
                    ui.text_edit_singleline(&mut def.texture);
                    if let Some(v) = vanilla {
                        if def.texture != v.texture && ui.button("↺").clicked() {
                            def.texture = v.texture.clone();
                        }
                    }
                },
            );
            field_row(
                ui,
                "Def:",
                vanilla.map(|v| def.def != v.def).unwrap_or(true),
                |ui| {
                    ui.text_edit_singleline(&mut def.def);
                    if let Some(v) = vanilla {
                        if def.def != v.def && ui.button("↺").clicked() {
                            def.def = v.def.clone();
                        }
                    }
                },
            );

            // Box / shadow dimensions
            for (label, val, van) in [
                (
                    "Box Width:",
                    &mut def.box_width,
                    vanilla.map(|v| v.box_width),
                ),
                (
                    "Box Height:",
                    &mut def.box_height,
                    vanilla.map(|v| v.box_height),
                ),
                (
                    "Box Sub Height:",
                    &mut def.box_sub_height,
                    vanilla.map(|v| v.box_sub_height),
                ),
                (
                    "Shadow Width:",
                    &mut def.shadow_width,
                    vanilla.map(|v| v.shadow_width),
                ),
                (
                    "Shadow Height:",
                    &mut def.shadow_height,
                    vanilla.map(|v| v.shadow_height),
                ),
            ] {
                let changed = van.map(|vv| *val != vv).unwrap_or(true);
                ui.horizontal(|ui| {
                    if changed {
                        ui.colored_label(CHANGED_COLOR, label);
                    } else {
                        ui.label(label);
                    }
                    ui.add(egui::DragValue::new(val));
                    if let Some(vv) = van {
                        if *val != vv && ui.button("↺").clicked() {
                            *val = vv;
                        }
                    }
                });
            }

            ui.separator();

            // Fields
            ui.collapsing(format!("Fields ({})", def.fields.len()), |ui| {
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .show(ui, |ui| {
                        for field in def.fields.iter_mut() {
                            let fname = format!(
                                "{}: {}",
                                field.id,
                                monster_names::get_monster_field_name(def.type_, field.id)
                            );
                            let changed = vanilla
                                .and_then(|vdef| vdef.fields.iter().find(|vf| vf.id == field.id))
                                .map(|vf| monster_values_differ(&field.value, &vf.value))
                                .unwrap_or(true);

                            let label = if changed {
                                egui::RichText::new(&fname).color(CHANGED_COLOR)
                            } else {
                                egui::RichText::new(&fname)
                            };

                            ui.horizontal(|ui| {
                                ui.label(label);
                                match &mut field.value {
                                    MonsterFieldValue::Float(v) => {
                                        ui.add(egui::DragValue::new(v).speed(0.1));
                                    }
                                    MonsterFieldValue::Int(v) => {
                                        ui.add(egui::DragValue::new(v));
                                    }
                                    MonsterFieldValue::String(v) => {
                                        ui.text_edit_singleline(v);
                                    }
                                }
                                if let Some(vanilla_def) = vanilla {
                                    if let Some(vf) =
                                        vanilla_def.fields.iter().find(|vf| vf.id == field.id)
                                    {
                                        if monster_values_differ(&field.value, &vf.value)
                                            && ui.button("↺").clicked()
                                        {
                                            field.value = vf.value.clone();
                                        }
                                    }
                                }
                            });
                        }
                    });
            });

            // Flags as checkboxes
            let flag_count = monster_names::get_monster_flag_count(def.type_);
            ui.collapsing(
                format!("Flags ({} active / {} total)", def.flags.len(), flag_count),
                |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            for i in 0..flag_count {
                                let is_set = def.flags.contains(&i);
                                let vanilla_set = vanilla.map(|v| v.flags.contains(&i));
                                let changed = vanilla_set.map(|vs| vs != is_set).unwrap_or(false);

                                let label = format!(
                                    "[{}] {}",
                                    i,
                                    monster_names::get_monster_flag_name(def.type_, i)
                                );
                                let label_rich = if changed {
                                    egui::RichText::new(&label).color(CHANGED_COLOR)
                                } else {
                                    egui::RichText::new(&label)
                                };

                                let mut checked = is_set;
                                if ui.checkbox(&mut checked, label_rich).changed() {
                                    if checked {
                                        if !def.flags.contains(&i) {
                                            def.flags.push(i);
                                            def.flags.sort_unstable();
                                        }
                                    } else {
                                        def.flags.retain(|&f| f != i);
                                    }
                                }
                            }

                            // Show any flags beyond the known range
                            let extra: Vec<i32> = def
                                .flags
                                .iter()
                                .copied()
                                .filter(|&f| f >= flag_count)
                                .collect();
                            if !extra.is_empty() {
                                ui.separator();
                                ui.label(egui::RichText::new("Unknown flags (raw):").italics());
                                for f in extra {
                                    ui.label(format!("  [{}] Unknown Flag", f));
                                }
                            }
                        });
                },
            );
        });
}
