use crate::app::ResalinatedApp;
use crate::atlas::HitboxPreview;
use crate::tabs::utils::{CHANGED_COLOR, field_row};
use eframe::egui;
use egui::Ui;
use sas2_parser::monster_catalog::{MonsterCatalog, MonsterDef, MonsterFieldValue};
use sas2_parser::monster_names;

/// Return a monster name not already used in the catalog (appends _1, _2, ... on collision).
fn next_unique_monster_name(catalog: &MonsterCatalog, base: &str) -> String {
    let exists = |n: &str| catalog.monsters.iter().any(|m| m.name == n);
    if !exists(base) {
        return base.to_string();
    }
    let mut i = 1;
    loop {
        let candidate = format!("{}_{}", base, i);
        if !exists(&candidate) {
            return candidate;
        }
        i += 1;
    }
}

/// Append a monster def, keep the name index in sync, select it, and clear filters so it shows.
fn add_monster(app: &mut ResalinatedApp, def: MonsterDef) {
    if let Some(cat) = app.working_monster_catalog.as_mut() {
        let idx = cat.monsters.len();
        cat.by_name.insert(def.name.clone(), idx as i32);
        cat.monsters.push(def);
        app.selected_monster_idx = Some(idx);
        app.monster_search_filter.clear();
        app.show_only_changed_monsters = false;
    }
}

/// Create a blank monster.
/// Title/description keep exactly 20 slots so the binary layout stays valid; everything else starts neutral and is filled in via the editor.
fn create_blank_monster(app: &mut ResalinatedApp) {
    let name = app
        .working_monster_catalog
        .as_ref()
        .map(|c| next_unique_monster_name(c, "new_monster"))
        .unwrap_or_else(|| "new_monster".to_string());

    let def = MonsterDef {
        name,
        titles: vec![String::new(); 20],
        descriptions: vec![String::new(); 20],
        type_: 0,
        sub_type: 0,
        cost: 0.0,
        img: -1,
        alt_img: -1,
        texture: String::new(),
        def: String::new(),
        box_width: 64,
        box_height: 96,
        box_sub_height: 0,
        shadow_width: 48,
        shadow_height: 16,
        fields: Vec::new(),
        flags: Vec::new(),
    };
    add_monster(app, def);
}

/// Remove the selected monster from the working catalog and reindex.
/// Vanilla monsters removed here are recorded as deletions on save, so they also drop out of the applied catalog.
fn delete_selected_monster(app: &mut ResalinatedApp) {
    let Some(idx) = app.selected_monster_idx else {
        return;
    };
    let removed_name = app
        .working_monster_catalog
        .as_ref()
        .and_then(|c| c.monsters.get(idx))
        .map(|d| d.name.clone());
    if let Some(cat) = app.working_monster_catalog.as_mut() {
        if idx >= cat.monsters.len() {
            return;
        }
        cat.monsters.remove(idx);
        cat.by_name.clear();
        for (i, d) in cat.monsters.iter().enumerate() {
            cat.by_name.insert(d.name.clone(), i as i32);
        }
    }
    if let Some(name) = removed_name {
        app.monster_disabled.remove(&name);
    }
    app.selected_monster_idx = None;
}

/// Clone the selected monster under a new unique "<name>_copy" name (full structure preserved).
fn clone_selected_monster(app: &mut ResalinatedApp) {
    // Clone every multi-selected monster (or just the single selection when none).
    let sources: Vec<usize> = if app.selected_monster_idxs.len() > 1 {
        let mut v: Vec<usize> = app.selected_monster_idxs.iter().copied().collect();
        v.sort_unstable();
        v
    } else {
        app.selected_monster_idx.into_iter().collect()
    };
    if sources.is_empty() {
        return;
    }
    let new_defs = {
        let Some(cat) = app.working_monster_catalog.as_ref() else {
            return;
        };
        sources
            .iter()
            .filter_map(|&idx| {
                let src = cat.monsters.get(idx)?;
                let mut def = src.clone();
                def.name = next_unique_monster_name(cat, &format!("{}_copy", src.name));
                Some(def)
            })
            .collect::<Vec<_>>()
    };
    for def in new_defs {
        add_monster(app, def);
    }
}

fn add_monster_label(ui: &mut Ui, title: &str, font_size: f32, selected: bool) {
    let color = if selected {
        egui::Color32::LIGHT_GREEN
    } else {
        ui.visuals().text_color()
    };
    for word in title.split_whitespace() {
        ui.add(
            egui::Label::new(egui::RichText::new(word).size(font_size).color(color))
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

    // Candidates for "Copy logic from" (name, display, type, subtype).
    let copy_candidates: Vec<(String, String, i32, i32)> = app
        .working_monster_catalog
        .as_ref()
        .map(|c| {
            c.monsters
                .iter()
                .map(|d| {
                    let display = d
                        .titles
                        .first()
                        .filter(|t| !t.is_empty())
                        .cloned()
                        .unwrap_or_else(|| d.name.clone());
                    (d.name.clone(), display, d.type_, d.sub_type)
                })
                .collect()
        })
        .unwrap_or_default();
    let mut request_copy_picker = false;

    // Assemble the selected monster's sprite (with origin) for the hitbox overlay, before the detail panel borrows the catalog mutably.
    let hitbox_preview: Option<HitboxPreview> = app.selected_monster_idx.and_then(|idx| {
        let names = app
            .working_monster_catalog
            .as_ref()
            .and_then(|c| c.monsters.get(idx))
            .map(|d| (d.def.clone(), d.texture.clone()));
        match names {
            Some((def_name, texture)) if !def_name.is_empty() && !texture.is_empty() => app
                .monster_texture_cache
                .get_idle_with_origin(ui.ctx(), &def_name, &texture),
            _ => None,
        }
    });

    // Right panel: editor
    let right_panel = egui::Panel::right("monster_details")
        .resizable(true)
        .default_size(panel_width)
        .min_size(min_size)
        .max_size(full_width * 0.8)
        .size_range(min_size..=full_width * 0.8)
        .show_inside(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // Multi-selection: edit the common fields of every selected monster.
            let multi: Vec<usize> = app.selected_monster_idxs.iter().copied().collect();
            if multi.len() > 1 {
                ui.heading("Edit Selected Monsters");
                ui.label(format!("{} monsters selected", multi.len()));
                ui.add_space(4.0);

                // Name / Cost apply to every selected monster.
                let first_name = app
                    .working_monster_catalog
                    .as_ref()
                    .and_then(|c| c.monsters.get(multi[0]))
                    .map(|d| d.name.clone())
                    .unwrap_or_default();
                let first_cost = app
                    .working_monster_catalog
                    .as_ref()
                    .and_then(|c| c.monsters.get(multi[0]))
                    .map(|d| d.cost)
                    .unwrap_or(0.0);

                let mut name = first_name;
                let mut cost = first_cost;
                let name_changed = ui
                    .horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut name).changed()
                    })
                    .inner;
                let cost_changed = ui
                    .horizontal(|ui| {
                        ui.label("Cost:");
                        ui.add(egui::DragValue::new(&mut cost).speed(1.0)).changed()
                    })
                    .inner;

                if name_changed || cost_changed {
                    if let Some(cat) = app.working_monster_catalog.as_mut() {
                        for &idx in &multi {
                            if let Some(d) = cat.monsters.get_mut(idx) {
                                if name_changed {
                                    d.name = name.clone();
                                }
                                if cost_changed {
                                    d.cost = cost;
                                }
                            }
                        }
                        if name_changed {
                            cat.by_name.clear();
                            for (i, d) in cat.monsters.iter().enumerate() {
                                cat.by_name.insert(d.name.clone(), i as i32);
                            }
                        }
                    }
                }
                ui.add_space(4.0);
                if ui.button("Remove all selected").clicked() {
                    let mut to_remove = multi.clone();
                    to_remove.sort_unstable();
                    to_remove.reverse();
                    if let Some(cat) = app.working_monster_catalog.as_mut() {
                        for idx in &to_remove {
                            if *idx < cat.monsters.len() {
                                let name = cat.monsters[*idx].name.clone();
                                cat.monsters.remove(*idx);
                                app.monster_disabled.remove(&name);
                            }
                        }
                        cat.by_name.clear();
                        for (i, d) in cat.monsters.iter().enumerate() {
                            cat.by_name.insert(d.name.clone(), i as i32);
                        }
                    }
                    app.selected_monster_idxs.clear();
                    app.selected_monster_idx = None;
                }
                ui.separator();
                // With a multi-selection active, hide the single-monster editor so it doesn't give the false impression that changes apply to one monster.
                return;
            }

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
                    show_monsterdef_editor(
                        ui,
                        def,
                        vanilla_def.as_ref(),
                        hitbox_preview.as_ref(),
                        &mut request_copy_picker,
                        &mut app.drops_clipboard,
                        &mut app.drops_copy_picker_open,
                        &mut app.copy_picker_search,
                        &mut app.copy_picker_focus,
                        &mut app.drop_item_picker_target,
                        &mut app.drop_item_picker_focus,
                        app.config.drag_value_sensitivity,
                        app.config.sidebar_font_size,
                    );
                } else {
                    ui.label("Invalid selection.");
                }
            } else {
                ui.label("Select a monster to edit.");
            }
        });

    if request_copy_picker {
        app.copy_picker_open = true;
        app.copy_picker_search.clear();
        app.copy_picker_focus = true;
    }

    // "Copy logic from" picker: searchable popup of same-type monsters.
    if app.copy_picker_open {
        let sel = app.selected_monster_idx.and_then(|idx| {
            app.working_monster_catalog
                .as_ref()
                .and_then(|c| c.monsters.get(idx))
                .map(|d| (d.name.clone(), d.type_, d.sub_type))
        });
        if let Some((sel_name, sel_type, sel_sub)) = sel {
            let mut chosen: Option<String> = None;
            let mut open = app.copy_picker_open;
            egui::Window::new("Copy logic from")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .open(&mut open)
                .show(ui.ctx(), |ui| {
                    ui.set_width(320.0);
                    ui.label(format!(
                        "Monsters of type {} ({})",
                        sel_type,
                        monster_names::get_monster_type_name(sel_type)
                    ));
                    ui.horizontal(|ui| {
                        ui.label("🔍");
                        let resp = ui.text_edit_singleline(&mut app.copy_picker_search);
                        if app.copy_picker_focus {
                            resp.request_focus();
                            app.copy_picker_focus = false;
                        }
                    });
                    ui.separator();

                    let needle = app.copy_picker_search.to_lowercase();
                    let mut matches: Vec<&(String, String, i32, i32)> = copy_candidates
                        .iter()
                        .filter(|(n, disp, t, _)| {
                            *t == sel_type
                                && n != &sel_name
                                && (needle.is_empty()
                                    || n.to_lowercase().contains(&needle)
                                    || disp.to_lowercase().contains(&needle))
                        })
                        .collect();
                    matches.sort_by_key(|(_, _, _, st)| (*st != sel_sub) as i32);

                    // Virtualized rows: only the rows visible in the scroll viewport are laid out each frame.
                    let row_height = ui
                        .text_style_height(&egui::TextStyle::Body)
                        .max(ui.spacing().interact_size.y);
                    egui::ScrollArea::vertical().max_height(400.0).show_rows(
                        ui,
                        row_height,
                        matches.len(),
                        |ui, row_range| {
                            for i in row_range {
                                let (n, disp, _, st) = matches[i];
                                let label = if *st == sel_sub {
                                    format!("{} ({})", disp, n)
                                } else {
                                    format!("{} ({}) [sub {}]", disp, n, st)
                                };
                                if ui
                                    .add(egui::Button::selectable(false, label).truncate())
                                    .clicked()
                                {
                                    chosen = Some(n.clone());
                                }
                            }
                        },
                    );
                });
            app.copy_picker_open = open;
            if let Some(src_name) = chosen {
                if let Some(idx) = app.selected_monster_idx {
                    apply_monster_copy_logic(app, idx, &src_name);
                }
                app.copy_picker_open = false;
            }
        } else {
            app.copy_picker_open = false;
        }
    }

    // "Copy drops (pick)" picker: same list as copy logic, but copies only the drops (fields 45-59) instead of the full logic.
    if app.drops_copy_picker_open {
        let sel = app.selected_monster_idx.and_then(|idx| {
            app.working_monster_catalog
                .as_ref()
                .and_then(|c| c.monsters.get(idx))
                .map(|d| (d.name.clone(), d.type_, d.sub_type))
        });
        if let Some((sel_name, sel_type, sel_sub)) = sel {
            let mut chosen: Option<String> = None;
            let mut open = app.drops_copy_picker_open;
            egui::Window::new("Copy drops from")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
                .open(&mut open)
                .show(ui.ctx(), |ui| {
                    ui.set_width(320.0);
                    ui.label(format!(
                        "Monsters of type {} ({})",
                        sel_type,
                        monster_names::get_monster_type_name(sel_type)
                    ));
                    ui.horizontal(|ui| {
                        ui.label("🔍");
                        let resp = ui.text_edit_singleline(&mut app.copy_picker_search);
                        if app.copy_picker_focus {
                            resp.request_focus();
                            app.copy_picker_focus = false;
                        }
                    });
                    ui.separator();

                    let needle = app.copy_picker_search.to_lowercase();
                    let mut matches: Vec<&(String, String, i32, i32)> = copy_candidates
                        .iter()
                        .filter(|(n, disp, t, _)| {
                            *t == sel_type
                                && n != &sel_name
                                && (needle.is_empty()
                                    || n.to_lowercase().contains(&needle)
                                    || disp.to_lowercase().contains(&needle))
                        })
                        .collect();
                    matches.sort_by_key(|(_, _, _, st)| (*st != sel_sub) as i32);

                    // Virtualized rows: only the rows visible in the scroll viewport are laid out each frame.
                    let row_height = ui
                        .text_style_height(&egui::TextStyle::Body)
                        .max(ui.spacing().interact_size.y);
                    egui::ScrollArea::vertical().max_height(400.0).show_rows(
                        ui,
                        row_height,
                        matches.len(),
                        |ui, row_range| {
                            for i in row_range {
                                let (n, disp, _, st) = matches[i];
                                let label = if *st == sel_sub {
                                    format!("{} ({})", disp, n)
                                } else {
                                    format!("{} ({}) [sub {}]", disp, n, st)
                                };
                                if ui
                                    .add(egui::Button::selectable(false, label).truncate())
                                    .clicked()
                                {
                                    chosen = Some(n.clone());
                                }
                            }
                        },
                    );
                });
            app.drops_copy_picker_open = open;
            if let Some(src_name) = chosen {
                if let Some(src_idx) = app
                    .working_monster_catalog
                    .as_ref()
                    .and_then(|c| c.by_name.get(&src_name).copied())
                {
                    copy_drops_from(app, src_idx as usize);
                }
                if let Some(idx) = app.selected_monster_idx {
                    paste_drops_to(app, idx);
                }
                app.drops_copy_picker_open = false;
            }
        } else {
            app.drops_copy_picker_open = false;
        }
    }

    // Item picker for a drop type field: grouped grid like the shop's add-item picker, with a materials-only toggle.
    if app.drop_item_picker_target.is_some() {
        let mut open = true;
        let mut chosen: Option<String> = None;
        egui::Window::new("Pick drop item")
            .collapsible(false)
            .resizable(true)
            .default_width(620.0)
            .default_height(480.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut app.drop_item_picker_materials_only, "Materials only");
                    ui.label("Search:");
                    let resp = ui.text_edit_singleline(&mut app.drop_item_picker_search);
                    if app.drop_item_picker_focus {
                        resp.request_focus();
                        app.drop_item_picker_focus = false;
                    }
                });
                ui.separator();

                let needle = app.drop_item_picker_search.to_lowercase();
                let loot = app.working_catalog.as_ref();
                let mut grouped: std::collections::BTreeMap<
                    String,
                    Vec<&sas2_parser::loot_catalog::LootDef>,
                > = std::collections::BTreeMap::new();
                if let Some(cat) = loot {
                    for d in &cat.loot_defs {
                        if app.drop_item_picker_materials_only && d.type_ != 4 {
                            continue;
                        }
                        let display = d
                            .title
                            .first()
                            .filter(|t| !t.is_empty())
                            .cloned()
                            .unwrap_or_else(|| d.name.clone());
                        if !needle.is_empty()
                            && !d.name.to_lowercase().contains(&needle)
                            && !display.to_lowercase().contains(&needle)
                        {
                            continue;
                        }
                        let cat_name = format!(
                            "{} - {}",
                            sas2_parser::loot_names::get_type_name(d.type_),
                            sas2_parser::loot_names::get_subtype_name(d.type_, d.sub_type)
                        );
                        grouped.entry(cat_name).or_default().push(d);
                    }
                }

                egui::ScrollArea::both()
                    .auto_shrink([false; 2])
                    .show_viewport(ui, |ui, viewport| {
                        // Only items whose x-range intersects the visible viewport are laid out each frame.
                        // Culled items still advance the grid cursor via allocate_space with the exact cell size, so positions, row heights and the scrollbar stay exact while the widget count stays proportional to the viewport.
                        let icon_size = app.config.item_icon_size;
                        let font_size = app.config.grid_font_size;
                        let label_h =
                            ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(font_size)));
                        let spacing_y = ui.spacing().item_spacing.y;
                        let pad_x = 2.0 * ui.spacing().button_padding.x;
                        let pad_y = 2.0 * ui.spacing().button_padding.y;
                        let overscan = 3.0 * (icon_size + pad_x);
                        let vp_min = viewport.min.x - overscan;
                        let vp_max = viewport.max.x + overscan;

                        for (cat_name, entries) in grouped {
                            ui.style_mut().interaction.selectable_labels = false;
                            ui.label(
                                egui::RichText::new(&cat_name)
                                    .strong()
                                    .size(app.config.category_font_size),
                            );
                            egui::Grid::new(("drop_pick_grid", cat_name))
                                .spacing([8.0, 8.0])
                                .show(ui, |ui| {
                                    let mut x = 0.0f32;
                                    for d in entries {
                                        let has_icon = app
                                            .item_atlas
                                            .as_ref()
                                            .and_then(|a| a.icon_uv(d))
                                            .is_some()
                                            || (app.image_editor.capacity as i32 > 0
                                                && d.img >= app.image_editor.capacity as i32
                                                && app.image_editor.icons.iter().any(
                                                    |(local, _)| {
                                                        *local
                                                            == d.img
                                                                - app.image_editor.capacity as i32
                                                    },
                                                ));
                                        let display = d
                                            .title
                                            .first()
                                            .filter(|t| !t.is_empty())
                                            .cloned()
                                            .unwrap_or_else(|| d.name.clone());
                                        let word_count = display.split_whitespace().count();
                                        // Image buttons are icon_size + button frame margins wide; placeholders are icon_size.
                                        let item_w = if has_icon {
                                            icon_size + pad_x
                                        } else {
                                            icon_size
                                        };
                                        let item_h = if has_icon {
                                            icon_size + pad_y
                                        } else {
                                            icon_size
                                        } + word_count as f32 * (label_h + spacing_y);
                                        let start = x;
                                        let end = x + item_w;
                                        x = end + 8.0;
                                        if end < vp_min || start > vp_max {
                                            ui.allocate_space(egui::vec2(item_w, item_h));
                                            continue;
                                        }

                                        ui.vertical(|ui| {
                                            let response = crate::tabs::items::draw_image_button(
                                                ui,
                                                app.item_atlas.as_ref(),
                                                Some(d),
                                                icon_size,
                                                &app.image_editor,
                                            );
                                            if response.clicked() {
                                                chosen = Some(d.name.clone());
                                            }
                                            crate::tabs::items::add_item_label(
                                                ui, &display, font_size, false,
                                            );
                                        });
                                    }
                                });
                            ui.add_space(8.0);
                        }
                    });
            });

        if let Some(name) = chosen {
            if let (Some(idx), Some(field_id)) =
                (app.selected_monster_idx, app.drop_item_picker_target)
            {
                if let Some(cat) = app.working_monster_catalog.as_mut() {
                    if let Some(def) = cat.monsters.get_mut(idx) {
                        if let Some(field) = def.fields.iter_mut().find(|f| f.id == field_id) {
                            field.value = MonsterFieldValue::String(name);
                        }
                    }
                }
            }
            app.drop_item_picker_target = None;
            app.drop_item_picker_search.clear();
        }
        if !open {
            app.drop_item_picker_target = None;
            app.drop_item_picker_search.clear();
        }
    }

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
            crate::tabs::multisel::mouse_help_button(ui, &[]);
        });
        ui.checkbox(
            &mut app.show_only_changed_monsters,
            "Show Only Changed Monsters",
        );

        // Create / clone monsters.
        ui.horizontal(|ui| {
            if ui.button("New Monster").clicked() {
                create_blank_monster(app);
            }
            let has_sel = app.selected_monster_idx.is_some();
            if ui
                .add_enabled(has_sel, egui::Button::new("Clone Selected"))
                .on_hover_text("Duplicate the selected monster(s) under new unique names")
                .clicked()
            {
                clone_selected_monster(app);
            }

            // Multi-select: disable/enable/delete selected / remove all by type.
            let multi_count = app.selected_monster_idxs.len();
            if multi_count > 1 || !app.monsters_remove_all_open {
                if multi_count > 1 {
                    // Disable / Enable selected: toggles the disabled set for all selected.
                    let all_disabled = app.selected_monster_idxs.iter().all(|&idx| {
                        app.working_monster_catalog
                            .as_ref()
                            .and_then(|c| c.monsters.get(idx))
                            .is_some_and(|d| app.monster_disabled.contains(&d.name))
                    });
                    if all_disabled {
                        if ui
                            .button(format!("Enable selected ({})", multi_count))
                            .on_hover_text("Re-include the selected monsters in the game")
                            .clicked()
                        {
                            for &idx in &app.selected_monster_idxs {
                                if let Some(d) = app
                                    .working_monster_catalog
                                    .as_ref()
                                    .and_then(|c| c.monsters.get(idx))
                                {
                                    app.monster_disabled.remove(&d.name);
                                }
                            }
                        }
                    } else if ui
                        .button(format!("Disable selected ({})", multi_count))
                        .on_hover_text("Exclude the selected monsters from the game but keep them (re-enableable)")
                        .clicked()
                    {
                        for &idx in &app.selected_monster_idxs {
                            if let Some(d) = app
                                .working_monster_catalog
                                .as_ref()
                                .and_then(|c| c.monsters.get(idx))
                            {
                                app.monster_disabled.insert(d.name.clone());
                            }
                        }
                    }
                    // Delete selected: only non-vanilla monsters that are already disabled, mirroring the single-item delete rule.
                    let deletable: Vec<usize> = app
                        .selected_monster_idxs
                        .iter()
                        .copied()
                        .filter(|&idx| {
                            app.working_monster_catalog
                                .as_ref()
                                .and_then(|c| c.monsters.get(idx))
                                .is_some_and(|d| {
                                    !app.vanilla_monster_catalog.as_ref().map_or(false, |v| {
                                        v.by_name.contains_key(&d.name)
                                    }) && app.monster_disabled.contains(&d.name)
                                })
                        })
                        .collect();
                    if !deletable.is_empty()
                        && ui
                            .button(format!("Delete selected ({})", deletable.len()))
                            .on_hover_text("Permanently remove the selected non-vanilla monsters (they must be disabled first)")
                            .clicked()
                    {
                        let mut to_remove = deletable;
                        to_remove.sort_unstable();
                        to_remove.reverse();
                        if let Some(cat) = app.working_monster_catalog.as_mut() {
                            for idx in &to_remove {
                                if *idx < cat.monsters.len() {
                                    let name = cat.monsters[*idx].name.clone();
                                    cat.monsters.remove(*idx);
                                    app.monster_disabled.remove(&name);
                                }
                            }
                            cat.by_name.clear();
                            for (i, d) in cat.monsters.iter().enumerate() {
                                cat.by_name.insert(d.name.clone(), i as i32);
                            }
                        }
                        app.selected_monster_idxs.clear();
                        app.selected_monster_idx = None;
                    }
                }
                if ui
                    .button("Remove all by type...")
                    .on_hover_text("Open a picker to remove every monster of the chosen type-subtype categories")
                    .clicked()
                {
                    app.monsters_remove_all_open = true;
                    app.monsters_remove_all_types.clear();
                }
            }

            // Disable (reversible) / Enable / Delete, mirroring the Items tab.
            let sel = app.selected_monster_idx.and_then(|idx| {
                app.working_monster_catalog
                    .as_ref()
                    .and_then(|c| c.monsters.get(idx))
                    .map(|d| d.name.clone())
            });
            if let Some(name) = sel {
                let is_vanilla = app
                    .vanilla_monster_catalog
                    .as_ref()
                    .map_or(false, |v| v.by_name.contains_key(&name));
                let is_disabled = app.monster_disabled.contains(&name);
                if is_disabled {
                    if ui
                        .button("Enable")
                        .on_hover_text("Re-include this monster in the game")
                        .clicked()
                    {
                        app.monster_disabled.remove(&name);
                    }
                    if !is_vanilla
                        && ui
                            .button("Delete")
                            .on_hover_text("Permanently remove this non-vanilla monster")
                            .clicked()
                    {
                        delete_selected_monster(app);
                    }
                } else if ui
                    .button("Disable")
                    .on_hover_text("Exclude from the game but keep it (re-enableable)")
                    .clicked()
                {
                    app.monster_disabled.insert(name);
                }
            }
        });
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

        // Selection gesture state (click / ctrl+click / shift+click / shift+drag box).
        let mut gsel = std::mem::take(&mut app.monsters_grid_sel);
        gsel.begin(ui);

        // Full display order (all filtered monsters, not just visible ones) so shift+click ranges work across scrolled-out entries.
        gsel.display_order.clear();
        for cat in &categories {
            for (idx, _) in &grouped[cat] {
                gsel.display_order.push(*idx);
            }
        }

        egui::ScrollArea::both()
            .scroll_source(crate::tabs::multisel::grid_scroll_source(ui))
            .auto_shrink([false; 2])
            .show_viewport(ui, |ui, viewport| {
                // Only monsters whose x-range intersects the visible viewport are laid out each frame.
                // Culled monsters still advance the grid cursor via allocate_space with the exact cell size, so positions, row heights and the scrollbar stay exact while the widget count stays proportional to the viewport.
                let icon_size = app.config.item_icon_size;
                let font_size = app.config.grid_font_size;
                let label_h = ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(font_size)));
                let spacing_y = ui.spacing().item_spacing.y;
                let pad_x = 2.0 * ui.spacing().button_padding.x;
                let pad_y = 2.0 * ui.spacing().button_padding.y;
                let overscan = 3.0 * (icon_size + pad_x);
                let vp_min = viewport.min.x - overscan;
                let vp_max = viewport.max.x + overscan;

                for cat in categories {
                    let entries = grouped.get(&cat).unwrap();
                    ui.style_mut().interaction.selectable_labels = false;
                    ui.label(
                        egui::RichText::new(&cat)
                            .strong()
                            .size(app.config.category_font_size),
                    );

                    egui::Grid::new(&cat).spacing([8.0, 8.0]).show(ui, |ui| {
                        let mut x = 0.0f32;
                        for (orig_idx, def) in entries {
                            let has_icon = !def.texture.is_empty();
                            let display_name = def
                                .titles
                                .first()
                                .filter(|t| !t.is_empty())
                                .cloned()
                                .unwrap_or_else(|| def.name.clone());
                            let word_count = display_name.split_whitespace().count();
                            // Image buttons are icon_size + button frame margins wide; placeholders are icon_size.
                            let item_w = if has_icon { icon_size + pad_x } else { icon_size };
                            let item_h = if has_icon { icon_size + pad_y } else { icon_size }
                                + word_count as f32 * (label_h + spacing_y);
                            let start = x;
                            let end = x + item_w;
                            x = end + 8.0;
                            if end < vp_min || start > vp_max {
                                ui.allocate_space(egui::vec2(item_w, item_h));
                                continue;
                            }

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
                                            egui::vec2(icon_size, icon_size),
                                        ),
                                    ))
                                } else {
                                    // placeholder while loading
                                    ui.allocate_response(
                                        egui::vec2(icon_size, icon_size),
                                        egui::Sense::click(),
                                    )
                                };
                                let btn_w = response.rect.width();
                                gsel.cell(response.rect, *orig_idx);
                                let is_sel = app.selected_monster_idx == Some(*orig_idx)
                                    || app.selected_monster_idxs.contains(orig_idx)
                                    || gsel.is_box_hit(orig_idx);
                                crate::tabs::multisel::paint_sel_outline(
                                    ui,
                                    response.rect,
                                    is_sel,
                                );
                                ui.set_max_width(btn_w);
                                add_monster_label(
                                    ui,
                                    &display_name,
                                    font_size,
                                    is_sel,
                                );
                                if app.monster_disabled.contains(&def.name) {
                                    ui.label(
                                        egui::RichText::new("(disabled)")
                                            .color(egui::Color32::from_rgb(220, 120, 120)),
                                    );
                                }
                            });
                        }
                    });

                    ui.add_space(8.0);
                }

                gsel.update_target();
                gsel.paint(ui);
                gsel.end(ui, &mut app.selected_monster_idxs, &mut app.selected_monster_idx);
                app.monsters_grid_sel = gsel;
            });
    });

    // "Remove all by type" picker window.
    if app.monsters_remove_all_open {
        let mut open = app.monsters_remove_all_open;
        let mut do_remove = false;
        egui::Window::new("Remove all by type")
            .collapsible(false)
            .resizable(true)
            .default_width(360.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                let mut cats: Vec<String> = Vec::new();
                if let Some(cat) = app.working_monster_catalog.as_ref() {
                    for d in &cat.monsters {
                        let c = format!(
                            "{} - SubType {}",
                            monster_names::get_monster_type_name(d.type_),
                            d.sub_type
                        );
                        if !cats.contains(&c) {
                            cats.push(c);
                        }
                    }
                }
                cats.sort();
                for cat in &cats {
                    let mut checked = app.monsters_remove_all_types.contains(cat);
                    if ui.checkbox(&mut checked, cat).changed() {
                        if checked {
                            app.monsters_remove_all_types.insert(cat.clone());
                        } else {
                            app.monsters_remove_all_types.remove(cat);
                        }
                    }
                }
                ui.separator();
                if ui
                    .add_enabled(
                        !app.monsters_remove_all_types.is_empty(),
                        egui::Button::new("Remove"),
                    )
                    .clicked()
                {
                    do_remove = true;
                }
            });
        app.monsters_remove_all_open = open;
        if do_remove {
            let mut to_remove: Vec<usize> = Vec::new();
            let mut vanilla_blocked = 0usize;
            if let Some(cat) = app.working_monster_catalog.as_ref() {
                for (idx, d) in cat.monsters.iter().enumerate() {
                    let c = format!(
                        "{} - SubType {}",
                        monster_names::get_monster_type_name(d.type_),
                        d.sub_type
                    );
                    if app.monsters_remove_all_types.contains(&c) {
                        // Vanilla monsters can only be disabled, never removed.
                        if app
                            .vanilla_monster_catalog
                            .as_ref()
                            .map_or(false, |v| v.by_name.contains_key(&d.name))
                        {
                            vanilla_blocked += 1;
                            app.monster_disabled.insert(d.name.clone());
                        } else {
                            to_remove.push(idx);
                        }
                    }
                }
            }
            if vanilla_blocked > 0 {
                app.error_message = Some(format!(
                    "{} vanilla monster(s) were disabled instead of removed (vanilla monsters cannot be deleted).",
                    vanilla_blocked
                ));
            }
            to_remove.sort_unstable();
            to_remove.reverse();
            if let Some(cat) = app.working_monster_catalog.as_mut() {
                for idx in &to_remove {
                    if *idx < cat.monsters.len() {
                        let name = cat.monsters[*idx].name.clone();
                        cat.monsters.remove(*idx);
                        app.monster_disabled.remove(&name);
                    }
                }
                cat.by_name.clear();
                for (i, d) in cat.monsters.iter().enumerate() {
                    cat.by_name.insert(d.name.clone(), i as i32);
                }
            }
            app.selected_monster_idxs.clear();
            app.selected_monster_idx = None;
            app.monsters_remove_all_open = false;
        }
    }
}

fn monster_values_differ(a: &MonsterFieldValue, b: &MonsterFieldValue) -> bool {
    match (a, b) {
        (MonsterFieldValue::Float(x), MonsterFieldValue::Float(y)) => (x - y).abs() > 0.001,
        (MonsterFieldValue::Int(x), MonsterFieldValue::Int(y)) => x != y,
        (MonsterFieldValue::String(x), MonsterFieldValue::String(y)) => x != y,
        _ => true,
    }
}

/// Replace the monster at `idx`'s fields and flags with those of `src_name`.
fn apply_monster_copy_logic(app: &mut ResalinatedApp, idx: usize, src_name: &str) {
    let src = app
        .working_monster_catalog
        .as_ref()
        .and_then(|c| c.monsters.iter().find(|d| d.name == src_name))
        .map(|d| (d.fields.clone(), d.flags.clone()));
    if let Some((fields, flags)) = src {
        if let Some(cat) = app.working_monster_catalog.as_mut() {
            if let Some(def) = cat.monsters.get_mut(idx) {
                def.fields = fields;
                def.flags = flags;
            }
        }
    }
}

/// Drop tier field triples (Type / Prob / Count) for a monster type.
/// Monsters (type 1) use fields 45-59; harvest (type 5) uses fields 0-14.
fn drop_tiers(monster_type: i32) -> Vec<(i32, i32, i32)> {
    let base = if monster_type == 5 { 0 } else { 45 };
    (0..5)
        .map(|i| (base + i * 3, base + i * 3 + 1, base + i * 3 + 2))
        .collect()
}

/// Drop field ids for a monster type: five loot tiers (Type / Prob / Count).
fn drop_field_ids(monster_type: i32) -> Vec<i32> {
    drop_tiers(monster_type)
        .into_iter()
        .flat_map(|(t, p, c)| [t, p, c])
        .collect()
}

/// Copy the drops (five loot tiers) of `def` into the drops clipboard.
fn copy_drops_from_def(
    def: &MonsterDef,
    clipboard: &mut Option<crate::magic_slot::DropsClipboard>,
) {
    let ids = drop_field_ids(def.type_);
    let fields: Vec<(i32, MonsterFieldValue)> = def
        .fields
        .iter()
        .filter(|f| ids.contains(&f.id))
        .map(|f| (f.id, f.value.clone()))
        .collect();
    let source = def
        .titles
        .first()
        .filter(|t| !t.is_empty())
        .cloned()
        .unwrap_or_else(|| def.name.clone());
    *clipboard = Some(crate::magic_slot::DropsClipboard {
        fields,
        source: Some(source),
    });
}

/// Apply the drops clipboard to `def`.
fn paste_drops_to_def(def: &mut MonsterDef, clip: &crate::magic_slot::DropsClipboard) {
    let ids = drop_field_ids(def.type_);
    for f in &mut def.fields {
        if ids.contains(&f.id) {
            if let Some((_, v)) = clip.fields.iter().find(|(id, _)| *id == f.id) {
                f.value = v.clone();
            }
        }
    }
}

/// Reset all drops on `def` to vanilla.
fn default_drops_on_def(def: &mut MonsterDef, vanilla: Option<&MonsterDef>) {
    let ids = drop_field_ids(def.type_);
    let vanilla_fields: Vec<(i32, MonsterFieldValue)> = vanilla
        .map(|v| {
            v.fields
                .iter()
                .filter(|f| ids.contains(&f.id))
                .map(|f| (f.id, f.value.clone()))
                .collect()
        })
        .unwrap_or_default();
    for f in &mut def.fields {
        if ids.contains(&f.id) {
            if let Some((_, v)) = vanilla_fields.iter().find(|(id, _)| *id == f.id) {
                f.value = v.clone();
            }
        }
    }
}

/// Copy the drops of the monster at `src_idx` into the drops clipboard.
fn copy_drops_from(app: &mut ResalinatedApp, src_idx: usize) {
    let Some(cat) = app.working_monster_catalog.as_ref() else {
        return;
    };
    let Some(src) = cat.monsters.get(src_idx) else {
        return;
    };
    copy_drops_from_def(src, &mut app.drops_clipboard);
}

/// Apply the drops clipboard to the monster at `idx`.
fn paste_drops_to(app: &mut ResalinatedApp, idx: usize) {
    let Some(clip) = app.drops_clipboard.clone() else {
        return;
    };
    let Some(cat) = app.working_monster_catalog.as_mut() else {
        return;
    };
    let Some(def) = cat.monsters.get_mut(idx) else {
        return;
    };
    paste_drops_to_def(def, &clip);
}

fn show_monsterdef_editor(
    ui: &mut Ui,
    def: &mut MonsterDef,
    vanilla: Option<&MonsterDef>,
    hitbox_preview: Option<&HitboxPreview>,
    request_copy_picker: &mut bool,
    drops_clipboard: &mut Option<crate::magic_slot::DropsClipboard>,
    drops_copy_picker_open: &mut bool,
    copy_picker_search: &mut String,
    copy_picker_focus: &mut bool,
    drop_item_picker_target: &mut Option<i32>,
    drop_item_picker_focus: &mut bool,
    drag_speed: f32,
    sidebar_font_size: f32,
) {
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.style_mut().override_text_style = Some(egui::TextStyle::Body);
            ui.style_mut().text_styles.insert(
                egui::TextStyle::Body,
                egui::FontId::proportional(sidebar_font_size),
            );
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

            // Hitbox (the damage-receiving box).
            // Per the game's HitPoint.CheckPoint, a hit lands when the attack point is within [origin.X - boxWidth/2, origin.X + boxWidth/2] horizontally and [origin.Y - boxHeight, origin.Y + boxSubHeight] vertically, where origin is the monster's ground position.
            // The schematic below shows that box to scale.
            ui.separator();
            ui.horizontal(|ui| {
                ui.heading("Hitbox");
                ui.label("(?)").on_hover_text(
                    "Damage-receiving box, centered on the monster's ground origin.\n\
                     Width spans +/- BoxWidth/2.\n\
                     BoxHeight extends up from the origin, BoxSubHeight down.\n\
                     Shadow Width/Height size the ground shadow only.",
                );
            });
            draw_hurtbox_schematic(ui, def, hitbox_preview);

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

            // Drops: five loot tiers (Type / Prob / Count).
            // Monsters (type 1) use fields 45-59; harvest (type 5) uses fields 0-14.
            let tiers = drop_tiers(def.type_);
            let has_drops = def
                .fields
                .iter()
                .any(|f| tiers.iter().any(|(t, p, c)| f.id == *t || f.id == *p || f.id == *c));
            ui.collapsing("Drops", |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button("Default drops")
                        .on_hover_text("Set all drops on this monster back to vanilla")
                        .clicked()
                    {
                        default_drops_on_def(def, vanilla);
                    }
                    if ui
                        .button("Copy drops (pick)")
                        .on_hover_text("Open the monster picker, then copy the selected monster's drops onto this one")
                        .clicked()
                    {
                        *drops_copy_picker_open = true;
                        copy_picker_search.clear();
                        *copy_picker_focus = true;
                    }
                    if ui
                        .button("Copy drops")
                        .on_hover_text("Copy this monster's drops to the clipboard")
                        .clicked()
                    {
                        copy_drops_from_def(def, drops_clipboard);
                    }
                    let has_clip = drops_clipboard.is_some();
                    let paste_resp = ui.add_enabled(has_clip, egui::Button::new("Paste drops"));
                    let paste_hover = if has_clip {
                        match &drops_clipboard.as_ref().unwrap().source {
                            Some(s) => format!("Apply the copied drops (from {}) to this monster", s),
                            None => "Apply the copied drops to this monster".to_string(),
                        }
                    } else {
                        "Copy drops first".to_string()
                    };
                    if paste_resp.on_hover_text(paste_hover).clicked() {
                        if let Some(clip) = drops_clipboard {
                            paste_drops_to_def(def, clip);
                        }
                    }
                });
                ui.separator();
                if !has_drops {
                    ui.label(
                        egui::RichText::new("No drop fields on this monster yet.")
                            ,
                    );
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(280.0)
                        .show(ui, |ui| {
                            for (ti, (type_id, prob_id, count_id)) in tiers.iter().enumerate() {
                                let changed = vanilla
                                    .map(|v| {
                                        let vf = |id| {
                                            v.fields
                                                .iter()
                                                .find(|f| f.id == id)
                                                .map(|f| &f.value)
                                        };
                                        let cf = |id| {
                                            def.fields
                                                .iter()
                                                .find(|f| f.id == id)
                                                .map(|f| &f.value)
                                        };
                                        match (
                                            cf(*type_id),
                                            vf(*type_id),
                                            cf(*prob_id),
                                            vf(*prob_id),
                                            cf(*count_id),
                                            vf(*count_id),
                                        ) {
                                            (Some(a), Some(b), Some(c), Some(d), Some(e), Some(f)) => {
                                                monster_values_differ(a, b)
                                                    || monster_values_differ(c, d)
                                                    || monster_values_differ(e, f)
                                            }
                                            _ => true,
                                        }
                                    })
                                    .unwrap_or(true);

                                let label = if changed {
                                    egui::RichText::new(format!("Item {}:", ti + 1))
                                        .color(CHANGED_COLOR)
                                } else {
                                    egui::RichText::new(format!("Item {}:", ti + 1))
                                };
                                ui.horizontal(|ui| {
                                    ui.label(label);
                                    for (fi, id) in [*type_id, *prob_id, *count_id]
                                        .iter()
                                        .enumerate()
                                    {
                                        let Some(field) =
                                            def.fields.iter_mut().find(|f| f.id == *id)
                                        else {
                                            continue;
                                        };
                                        // The tier header already says "Item N:", so the item
                                        // field needs no label; only Prob/Count get one.
                                        let field_label = match fi {
                                            1 => "Prob:",
                                            2 => "Count:",
                                            _ => "",
                                        };
                                        if !field_label.is_empty() {
                                            ui.label(field_label);
                                        }
                                        match &mut field.value {
                                            MonsterFieldValue::String(v) => {
                                                ui.add(
                                                    egui::TextEdit::singleline(v).desired_width(
                                                        ui.available_width() * 0.4,
                                                    ),
                                                );
                                                if fi == 0 {
                                                    if ui
                                                        .small_button("Pick")
                                                        .on_hover_text(
                                                            "Pick an item from the catalog",
                                                        )
                                                        .clicked()
                                                    {
                                                        *drop_item_picker_target = Some(*id);
                                                        *drop_item_picker_focus = true;
                                                    }
                                                }
                                            }
                                            MonsterFieldValue::Float(v) => {
                                                ui.add(
                                                    egui::DragValue::new(v)
                                                        .speed(drag_speed),
                                                );
                                            }
                                            MonsterFieldValue::Int(v) => {
                                                ui.add(egui::DragValue::new(v));
                                            }
                                        }
                                    }
                                });
                            }
                        });
                }
            });

            // Fields
            ui.collapsing(format!("Fields ({})", def.fields.len()), |ui| {
                // Copy logic (fields + flags) from another monster of the same type.
                if ui
                    .button("Copy logic from...")
                    .on_hover_text("Replace this monster's fields and flags with another monster's")
                    .clicked()
                {
                    *request_copy_picker = true;
                }

                let mut remove_field: Option<usize> = None;
                egui::ScrollArea::vertical()
                    .max_height(280.0)
                    .show(ui, |ui| {
                        for (field_index, field) in def.fields.iter_mut().enumerate() {
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
                                        ui.add(
                                            egui::DragValue::new(v)
                                                .speed(drag_speed),
                                        );
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
                                if ui
                                    .small_button("x")
                                    .on_hover_text("Remove this field")
                                    .clicked()
                                {
                                    remove_field = Some(field_index);
                                }
                            });
                        }
                    });

                if let Some(i) = remove_field {
                    if i < def.fields.len() {
                        def.fields.remove(i);
                    }
                }
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

/// Draw a to-scale schematic of the monster hurtbox (damage-receiving area) and ground shadow, with the monster's idle sprite rendered behind it so the box size can be judged against the actual entity.
///
/// Mirrors the in-game box from HitPoint.CheckPoint: horizontally centered on the origin spanning +/- boxWidth/2, vertically from origin - boxHeight (top) to origin + boxSubHeight (bottom).
/// Both the box and the sprite share the monster's ground origin and the same world-pixel scale.
fn draw_hurtbox_schematic(ui: &mut Ui, def: &MonsterDef, preview: Option<&HitboxPreview>) {
    use egui::{Color32, Pos2, Rect, Stroke};

    let bw = def.box_width.max(1) as f32;
    let bh = def.box_height.max(0) as f32;
    let bsh = def.box_sub_height.max(0) as f32;
    let sw = def.shadow_width.max(0) as f32;

    // Extents from the origin, expanded to contain both the box and (if present) the sprite.
    let mut ext_l = bw * 0.5;
    let mut ext_r = bw * 0.5;
    let mut ext_u = bh;
    let mut ext_d = bsh;
    if let Some(p) = preview {
        let (ox, oy) = p.origin;
        let (pw, ph) = (p.size.0 as f32, p.size.1 as f32);
        ext_l = ext_l.max(ox);
        ext_r = ext_r.max(pw - ox);
        ext_u = ext_u.max(oy);
        ext_d = ext_d.max(ph - oy);
    }
    let total_w = (ext_l + ext_r).max(1.0);
    let total_h = (ext_u + ext_d).max(1.0);

    let area = egui::vec2(ui.available_width().min(260.0), 200.0);
    let (rect, _) = ui.allocate_exact_size(area, egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_gray(28));

    let pad = 18.0;
    let scale = ((rect.width() - 2.0 * pad) / total_w).min((rect.height() - 2.0 * pad) / total_h);
    if !scale.is_finite() || scale <= 0.0 {
        return;
    }

    // Origin position inside the drawing area.
    let cx = rect.min.x + pad + ext_l * scale;
    let origin_y = rect.min.y + pad + ext_u * scale;

    // Sprite (behind the box).
    if let Some(p) = preview {
        let (ox, oy) = p.origin;
        let (pw, ph) = (p.size.0 as f32, p.size.1 as f32);
        let top_left = Pos2::new(cx - ox * scale, origin_y - oy * scale);
        let spr_rect = Rect::from_min_size(top_left, egui::vec2(pw * scale, ph * scale));
        painter.image(
            p.handle.id(),
            spr_rect,
            Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    // Hurtbox.
    let left = cx - bw * 0.5 * scale;
    let right = cx + bw * 0.5 * scale;
    let top = origin_y - bh * scale;
    let bottom = origin_y + bsh * scale;
    let box_rect = Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom));
    painter.rect_filled(
        box_rect,
        0.0,
        Color32::from_rgba_unmultiplied(220, 60, 60, 40),
    );
    painter.rect_stroke(
        box_rect,
        0.0,
        Stroke::new(1.5_f32, Color32::from_rgb(230, 80, 80)),
        egui::StrokeKind::Middle,
    );

    // Ground shadow (width only, drawn at the origin line).
    if sw > 0.0 {
        let half = sw * 0.5 * scale;
        painter.line_segment(
            [
                Pos2::new(cx - half, origin_y),
                Pos2::new(cx + half, origin_y),
            ],
            Stroke::new(3.0_f32, Color32::from_rgba_unmultiplied(120, 120, 180, 160)),
        );
    }

    // Origin marker (ground position).
    let oc = Color32::from_rgb(255, 220, 80);
    painter.line_segment(
        [Pos2::new(cx - 5.0, origin_y), Pos2::new(cx + 5.0, origin_y)],
        Stroke::new(1.5_f32, oc),
    );
    painter.line_segment(
        [Pos2::new(cx, origin_y - 5.0), Pos2::new(cx, origin_y + 5.0)],
        Stroke::new(1.5_f32, oc),
    );
}
