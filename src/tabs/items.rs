use crate::app::ResalinatedApp;
use crate::atlas::ItemAtlas;
use crate::magic_slot::MagicSlotOverrides;
use crate::tabs::utils::{CHANGED_COLOR, field_row};
use eframe::egui;
use egui::{Response, Ui};
use sas2_parser::loot_catalog::{LootCatalog, LootDef, LootField, LootFieldValue};
use sas2_parser::loot_names;
use std::collections::HashMap;

/// Draw one icon button.
/// Vanilla icons come from the items atlas
/// An icon at or beyond the vanilla capacity is a custom icon drawn from the custom-icon atlas.
/// A missing/empty icon renders an invisible placeholder so grid columns stay aligned.
pub fn draw_image_button(
    ui: &mut Ui,
    atlas: Option<&ItemAtlas>,
    def: Option<&LootDef>,
    icon_size: f32,
    image_editor: &crate::image_editor::ImageEditor,
) -> Response {
    // Custom icon?
    if let Some(d) = def {
        let cap = image_editor.capacity as i32;
        if cap > 0 && d.img >= cap {
            let local = d.img - cap;
            if let Some((_, handle)) = image_editor.icons.iter().find(|(i, _)| *i == local) {
                return ui.add(egui::Button::image(
                    egui::Image::from_texture(handle)
                        .fit_to_exact_size(egui::vec2(icon_size, icon_size)),
                ));
            }
        }
    }

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

/// Render a word-wrapped item name. When `selected`, the name is green.
pub fn add_item_label(ui: &mut Ui, title: &str, font_size: f32, selected: bool) {
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
    if app.working_catalog.is_none() {
        ui.label("No catalog loaded.");
        return;
    }

    // Load the custom-icon atlas info so the editor can offer vanilla vs custom icons.
    if let Some(gp) = app.game_path.clone() {
        app.image_editor.ensure_loaded(ui.ctx(), &gp);
    }
    let icon_capacity = app.image_editor.capacity as i32;

    let full_width = ui.available_width();
    let min_size = 250.0;
    let panel_width = if app.config.items_details_panel_width > 0.0 {
        app.config.items_details_panel_width.max(min_size)
    } else {
        full_width * 0.5
    };

    // Build magic items list once for the dropdown picker
    let mut magic_items: Vec<(String, String)> = app
        .working_catalog
        .as_ref()
        .map(|c| {
            c.loot_defs
                .iter()
                .filter(|d| d.type_ == 7)
                .map(|d| {
                    let display = d
                        .title
                        .first()
                        .filter(|t| !t.is_empty())
                        .cloned()
                        .unwrap_or_else(|| d.name.clone());
                    (d.name.clone(), display)
                })
                .collect()
        })
        .unwrap_or_default();

    // Sort by display text (second element)
    magic_items.sort_by(|a, b| a.1.cmp(&b.1));

    // Candidates for "Copy logic from": (name, display, type, subtype) for every item, so the editor can offer same-type items to copy fields/flags from
    // (e.g. give a new weapon the logic of an existing zweihander).
    let copy_candidates: Vec<(String, String, i32, i32)> = app
        .working_catalog
        .as_ref()
        .map(|c| {
            c.loot_defs
                .iter()
                .map(|d| {
                    let display = d
                        .title
                        .first()
                        .filter(|t| !t.is_empty())
                        .cloned()
                        .unwrap_or_else(|| d.name.clone());
                    (d.name.clone(), display, d.type_, d.sub_type)
                })
                .collect()
        })
        .unwrap_or_default();

    // Vanilla icon choices for the "Pick img" picker: distinct in-atlas img values with a representative item label (searchable by name or index).
    let mut icon_choices: Vec<(i32, String)> = Vec::new();
    {
        let mut seen: std::collections::HashSet<i32> = std::collections::HashSet::new();
        if let Some(c) = app.vanilla_catalog.as_ref() {
            for d in &c.loot_defs {
                if d.img >= 0 && d.img < icon_capacity && seen.insert(d.img) {
                    let label = d
                        .title
                        .first()
                        .filter(|t| !t.is_empty())
                        .cloned()
                        .unwrap_or_else(|| d.name.clone());
                    icon_choices.push((d.img, label));
                }
            }
        }
        icon_choices.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
    }

    // Right panel: item editor
    let right_panel = egui::Panel::right("item_details")
        .resizable(true)
        .default_size(panel_width)
        .min_size(min_size)
        .max_size(full_width * 0.8)
        .size_range(min_size..=full_width * 0.8)
        .show_inside(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // Multi-selection: edit the common fields of every selected item.
            let multi: Vec<usize> = app.selected_item_idxs.iter().copied().collect();
            if multi.len() > 1 {
                ui.heading("Edit Selected Items");
                ui.label(format!("{} items selected", multi.len()));
                ui.add_space(4.0);

                // Name / Cost / Token cost apply to every selected item.
                let first_name = app
                    .working_catalog
                    .as_ref()
                    .and_then(|c| c.loot_defs.get(multi[0]))
                    .map(|d| d.name.clone())
                    .unwrap_or_default();
                let first_cost = app
                    .working_catalog
                    .as_ref()
                    .and_then(|c| c.loot_defs.get(multi[0]))
                    .map(|d| d.cost)
                    .unwrap_or(0.0);
                let first_token = app
                    .working_catalog
                    .as_ref()
                    .and_then(|c| c.loot_defs.get(multi[0]))
                    .map(|d| d.token_cost)
                    .unwrap_or(0);

                let mut name = first_name;
                let mut cost = first_cost;
                let mut token_cost = first_token;
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
                let token_changed = ui
                    .horizontal(|ui| {
                        ui.label("Token cost:");
                        ui.add(egui::DragValue::new(&mut token_cost)).changed()
                    })
                    .inner;

                if name_changed || cost_changed || token_changed {
                    if let Some(cat) = app.working_catalog.as_mut() {
                        for &idx in &multi {
                            if let Some(d) = cat.loot_defs.get_mut(idx) {
                                if name_changed {
                                    d.name = name.clone();
                                }
                                if cost_changed {
                                    d.cost = cost;
                                }
                                if token_changed {
                                    d.token_cost = token_cost;
                                }
                            }
                        }
                        if name_changed {
                            cat.by_name.clear();
                            for (i, d) in cat.loot_defs.iter().enumerate() {
                                cat.by_name.insert(d.name.clone(), i);
                            }
                        }
                    }
                }

                // Mass-edit fields shared by all selected items.
                // Only field ids present in every selected item are shown, so nothing misleading appears.
                let shared_fields: Vec<(i32, String)> = {
                    let defs: Vec<&LootDef> = multi
                        .iter()
                        .filter_map(|&idx| {
                            app.working_catalog
                                .as_ref()
                                .and_then(|c| c.loot_defs.get(idx))
                        })
                        .collect();
                    if defs.is_empty() {
                        Vec::new()
                    } else {
                        let first = &defs[0];
                        first
                            .fields
                            .iter()
                            .filter(|f| {
                                defs.iter().all(|d| d.fields.iter().any(|df| df.id == f.id))
                            })
                            .map(|f| {
                                (
                                    f.id,
                                    loot_names::get_field_name(first.type_, f.id).to_string(),
                                )
                            })
                            .collect()
                    }
                };
                if !shared_fields.is_empty() {
                    ui.separator();
                    ui.label(egui::RichText::new("Shared Fields").strong());
                    for (fid, fname) in &shared_fields {
                        // Show the value from the first item as the editable seed.
                        let first_val = app
                            .working_catalog
                            .as_ref()
                            .and_then(|c| c.loot_defs.get(multi[0]))
                            .and_then(|d| d.fields.iter().find(|f| f.id == *fid))
                            .map(|f| f.value.clone());
                        let Some(first_val) = first_val else { continue };
                        let mut changed = false;
                        let mut new_val = first_val.clone();
                        ui.horizontal(|ui| {
                            ui.label(format!("{}:", fname));
                            match &mut new_val {
                                LootFieldValue::Float(v) => {
                                    changed = ui.add(egui::DragValue::new(v)).changed();
                                }
                                LootFieldValue::Int(v) => {
                                    changed = ui.add(egui::DragValue::new(v)).changed();
                                }
                                LootFieldValue::Bool(v) => {
                                    changed = ui.checkbox(v, "").changed();
                                }
                                LootFieldValue::String(v) => {
                                    changed = ui.text_edit_singleline(v).changed();
                                }
                            }
                        });
                        if changed {
                            if let Some(cat) = app.working_catalog.as_mut() {
                                for &idx in &multi {
                                    if let Some(d) = cat.loot_defs.get_mut(idx) {
                                        if let Some(f) = d.fields.iter_mut().find(|f| f.id == *fid)
                                        {
                                            f.value = new_val.clone();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                ui.add_space(4.0);
                if ui.button("Remove all selected").clicked() {
                    let mut to_remove = multi.clone();
                    to_remove.sort_unstable();
                    to_remove.reverse();
                    if let Some(cat) = app.working_catalog.as_mut() {
                        for idx in &to_remove {
                            if *idx < cat.loot_defs.len() {
                                let name = cat.loot_defs[*idx].name.clone();
                                cat.loot_defs.remove(*idx);
                                app.loot_disabled.remove(&name);
                                app.shop_additions.remove(&name);
                                app.craft_additions.remove(&name);
                            }
                        }
                        cat.by_name.clear();
                        for (i, d) in cat.loot_defs.iter().enumerate() {
                            cat.by_name.insert(d.name.clone(), i);
                        }
                        cat.black_starstone_index =
                            cat.loot_defs.iter().position(|d| d.name == "black_pearl");
                        cat.gray_starstone_index =
                            cat.loot_defs.iter().position(|d| d.name == "gray_pearl");
                    }
                    app.selected_item_idxs.clear();
                    app.selected_item_idx = None;
                }
                ui.separator();
                // With a multi-selection active, hide the single-item editor so it doesn't give the false impression that changes apply to one item.
                return;
            }

            show_lootdef_editor(
                app,
                ui,
                &magic_items,
                &copy_candidates,
                icon_capacity,
                &icon_choices,
            );
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
            crate::tabs::multisel::mouse_help_button(ui, &[]);
        });
        ui.checkbox(&mut app.show_only_changed_items, "Show Only Changed Items");

        // Create / clone items.
        ui.horizontal(|ui| {
            if ui.button("New Item").clicked() {
                create_blank_item(app);
            }
            let has_sel = app.selected_item_idx.is_some();
            if ui
                .add_enabled(has_sel, egui::Button::new("Clone Selected"))
                .on_hover_text("Duplicate the selected item(s) under new unique names")
                .clicked()
            {
                clone_selected_item(app);
            }

            // Multi-select: disable/enable/delete selected / remove all by type.
            let multi_count = app.selected_item_idxs.len();
            if multi_count > 1 || !app.items_remove_all_open {
                if multi_count > 1 {
                    // Disable / Enable selected: toggles the disabled set for all selected.
                    let all_disabled = app.selected_item_idxs.iter().all(|&idx| {
                        app.working_catalog
                            .as_ref()
                            .and_then(|c| c.loot_defs.get(idx))
                            .is_some_and(|d| app.loot_disabled.contains(&d.name))
                    });
                    if all_disabled {
                        if ui
                            .button(format!("Enable selected ({})", multi_count))
                            .on_hover_text("Re-include the selected items in the game")
                            .clicked()
                        {
                            for &idx in &app.selected_item_idxs {
                                if let Some(d) = app
                                    .working_catalog
                                    .as_ref()
                                    .and_then(|c| c.loot_defs.get(idx))
                                {
                                    app.loot_disabled.remove(&d.name);
                                }
                            }
                        }
                    } else if ui
                        .button(format!("Disable selected ({})", multi_count))
                        .on_hover_text("Exclude the selected items from the game but keep them (re-enableable)")
                        .clicked()
                    {
                        for &idx in &app.selected_item_idxs {
                            if let Some(d) = app
                                .working_catalog
                                .as_ref()
                                .and_then(|c| c.loot_defs.get(idx))
                            {
                                app.loot_disabled.insert(d.name.clone());
                            }
                        }
                    }
                    // Delete selected: only non-vanilla items that are already disabled, mirroring the single-item delete rule.
                    let deletable: Vec<usize> = app
                        .selected_item_idxs
                        .iter()
                        .copied()
                        .filter(|&idx| {
                            app.working_catalog
                                .as_ref()
                                .and_then(|c| c.loot_defs.get(idx))
                                .is_some_and(|d| {
                                    !app.vanilla_catalog.as_ref().map_or(false, |v| {
                                        v.by_name.contains_key(&d.name)
                                    }) && app.loot_disabled.contains(&d.name)
                                })
                        })
                        .collect();
                    if !deletable.is_empty()
                        && ui
                            .button(format!("Delete selected ({})", deletable.len()))
                            .on_hover_text("Permanently remove the selected non-vanilla items (they must be disabled first)")
                            .clicked()
                    {
                        let mut to_remove = deletable;
                        to_remove.sort_unstable();
                        to_remove.reverse();
                        if let Some(cat) = app.working_catalog.as_mut() {
                            for idx in &to_remove {
                                if *idx < cat.loot_defs.len() {
                                    let name = cat.loot_defs[*idx].name.clone();
                                    cat.loot_defs.remove(*idx);
                                    app.loot_disabled.remove(&name);
                                    app.shop_additions.remove(&name);
                                    app.craft_additions.remove(&name);
                                }
                            }
                            cat.by_name.clear();
                            for (i, d) in cat.loot_defs.iter().enumerate() {
                                cat.by_name.insert(d.name.clone(), i);
                            }
                            cat.black_starstone_index =
                                cat.loot_defs.iter().position(|d| d.name == "black_pearl");
                            cat.gray_starstone_index =
                                cat.loot_defs.iter().position(|d| d.name == "gray_pearl");
                        }
                        app.selected_item_idxs.clear();
                        app.selected_item_idx = None;
                    }
                }
                if ui
                    .button("Remove all by type...")
                    .on_hover_text(
                        "Open a picker to remove every item of the chosen type-subtype categories",
                    )
                    .clicked()
                {
                    app.items_remove_all_open = true;
                    app.items_remove_all_types.clear();
                }
            }

            // Disable (reversible) / Enable / Delete.
            // Vanilla items can only be disabled, a non-vanilla item must be disabled first, after which Delete (permanent) appears.
            let sel = app.selected_item_idx.and_then(|idx| {
                app.working_catalog
                    .as_ref()
                    .and_then(|c| c.loot_defs.get(idx))
                    .map(|d| d.name.clone())
            });
            if let Some(name) = sel {
                let is_vanilla = app
                    .vanilla_catalog
                    .as_ref()
                    .map_or(false, |v| v.by_name.contains_key(&name));
                let is_disabled = app.loot_disabled.contains(&name);
                if is_disabled {
                    if ui
                        .button("Enable")
                        .on_hover_text("Re-include this item in the game")
                        .clicked()
                    {
                        app.loot_disabled.remove(&name);
                    }
                    if !is_vanilla
                        && ui
                            .button("Delete")
                            .on_hover_text("Permanently remove this non-vanilla item")
                            .clicked()
                    {
                        delete_selected_item(app);
                    }
                } else if ui
                    .button("Disable")
                    .on_hover_text("Exclude from the game but keep it (re-enableable)")
                    .clicked()
                {
                    app.loot_disabled.insert(name);
                }
            }
        });
        ui.add_space(4.0);

        let filter = app.search_filter.to_lowercase();
        let vanilla = app.vanilla_catalog.as_ref();

        // Build filtered list
        let filtered_indices: Vec<(usize, &LootDef)> = app
            .working_catalog
            .as_ref()
            .unwrap()
            .loot_defs
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                let matches_search = d.name.to_lowercase().contains(&filter)
                    || d.title
                        .first()
                        .map(|t| t.to_lowercase().contains(&filter))
                        .unwrap_or(false);
                if !matches_search {
                    return false;
                }
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

        // Selection gesture state (click / ctrl+click / shift+click / shift+drag box).
        let mut gsel = std::mem::take(&mut app.items_grid_sel);
        gsel.begin(ui);

        // Full display order (all filtered items, not just visible ones) so
        // shift+click ranges work across scrolled-out items.
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
                            let has_icon = app
                                .item_atlas
                                .as_ref()
                                .and_then(|a| a.icon_uv(def))
                                .is_some()
                                || (app.image_editor.capacity as i32 > 0
                                    && def.img >= app.image_editor.capacity as i32
                                    && app.image_editor.icons.iter().any(|(local, _)| {
                                        *local == def.img - app.image_editor.capacity as i32
                                    }));
                            let display_name = def
                                .title
                                .first()
                                .filter(|t| !t.is_empty())
                                .cloned()
                                .unwrap_or_else(|| def.name.clone());
                            let word_count = display_name.split_whitespace().count();
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
                                let response = draw_image_button(
                                    ui,
                                    app.item_atlas.as_ref(),
                                    Some(def),
                                    icon_size,
                                    &app.image_editor,
                                );
                                let btn_w = response.rect.width();

                                gsel.cell(response.rect, *orig_idx);

                                let is_sel = app.selected_item_idx == Some(*orig_idx)
                                    || app.selected_item_idxs.contains(orig_idx)
                                    || gsel.is_box_hit(orig_idx);
                                crate::tabs::multisel::paint_sel_outline(ui, response.rect, is_sel);
                                ui.set_max_width(btn_w);
                                add_item_label(ui, &display_name, font_size, is_sel);
                                if app.loot_disabled.contains(&def.name) {
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
                gsel.end(ui, &mut app.selected_item_idxs, &mut app.selected_item_idx);
                app.items_grid_sel = gsel;
            });
    });

    // "Remove all by type" picker window.
    if app.items_remove_all_open {
        let mut open = app.items_remove_all_open;
        let mut do_remove = false;
        egui::Window::new("Remove all by type")
            .collapsible(false)
            .resizable(true)
            .default_width(360.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                let mut cats: Vec<String> = Vec::new();
                if let Some(cat) = app.working_catalog.as_ref() {
                    for d in &cat.loot_defs {
                        let c = format!(
                            "{} - {}",
                            loot_names::get_type_name(d.type_),
                            loot_names::get_subtype_name(d.type_, d.sub_type)
                        );
                        if !cats.contains(&c) {
                            cats.push(c);
                        }
                    }
                }
                cats.sort();
                crate::tabs::multisel::category_checkboxes(
                    ui,
                    &cats,
                    &mut app.items_remove_all_types,
                );
                ui.separator();
                if ui
                    .add_enabled(
                        !app.items_remove_all_types.is_empty(),
                        egui::Button::new("Remove"),
                    )
                    .clicked()
                {
                    do_remove = true;
                }
            });
        app.items_remove_all_open = open;
        if do_remove {
            let mut to_remove: Vec<usize> = Vec::new();
            let mut vanilla_blocked = 0usize;
            if let Some(cat) = app.working_catalog.as_ref() {
                for (idx, d) in cat.loot_defs.iter().enumerate() {
                    let c = format!(
                        "{} - {}",
                        loot_names::get_type_name(d.type_),
                        loot_names::get_subtype_name(d.type_, d.sub_type)
                    );
                    if app.items_remove_all_types.contains(&c) {
                        // Vanilla items can only be disabled, never removed.
                        if app
                            .vanilla_catalog
                            .as_ref()
                            .map_or(false, |v| v.by_name.contains_key(&d.name))
                        {
                            vanilla_blocked += 1;
                            app.loot_disabled.insert(d.name.clone());
                        } else {
                            to_remove.push(idx);
                        }
                    }
                }
            }
            if vanilla_blocked > 0 {
                app.error_message = Some(format!(
                    "{} vanilla item(s) were disabled instead of removed (vanilla items cannot be deleted).",
                    vanilla_blocked
                ));
            }
            to_remove.sort_unstable();
            to_remove.reverse();
            if let Some(cat) = app.working_catalog.as_mut() {
                for idx in &to_remove {
                    if *idx < cat.loot_defs.len() {
                        let name = cat.loot_defs[*idx].name.clone();
                        cat.loot_defs.remove(*idx);
                        app.loot_disabled.remove(&name);
                        app.shop_additions.remove(&name);
                        app.craft_additions.remove(&name);
                    }
                }
                cat.by_name.clear();
                for (i, d) in cat.loot_defs.iter().enumerate() {
                    cat.by_name.insert(d.name.clone(), i);
                }
                cat.black_starstone_index =
                    cat.loot_defs.iter().position(|d| d.name == "black_pearl");
                cat.gray_starstone_index =
                    cat.loot_defs.iter().position(|d| d.name == "gray_pearl");
            }
            app.selected_item_idxs.clear();
            app.selected_item_idx = None;
            app.items_remove_all_open = false;
        }
    }
}

/// Return a name not already used by any loot def (appends _1, _2, ... on collision).
fn next_unique_name(catalog: &LootCatalog, base: &str) -> String {
    let exists = |n: &str| catalog.loot_defs.iter().any(|d| d.name == n);
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

/// Append a new loot def to the working catalog, keep the name index in sync, and select it.
fn add_item(app: &mut ResalinatedApp, def: LootDef) {
    if let Some(cat) = app.working_catalog.as_mut() {
        let idx = cat.loot_defs.len();
        cat.by_name.insert(def.name.clone(), idx);
        cat.loot_defs.push(def);
        app.selected_item_idx = Some(idx);
        // Clear the filter so the newly added item is visible in the list.
        app.search_filter.clear();
        app.show_only_changed_items = false;
    }
}

/// Create a blank item from scratch.
/// Title/description must keep exactly 20 slots so the binary layout stays valid, fields/flags start empty and are filled in by the editor afterwards.
fn create_blank_item(app: &mut ResalinatedApp) {
    let name = app
        .working_catalog
        .as_ref()
        .map(|c| next_unique_name(c, "new_item"))
        .unwrap_or_else(|| "new_item".to_string());

    let mut def = LootDef {
        id: 0,
        name,
        title: vec![String::new(); 20],
        description: vec![String::new(); 20],
        type_: 0,
        sub_type: 0,
        cost: 0.0,
        img: -1,
        alt_img: -1,
        texture: String::new(),
        fields: Vec::new(),
        flags: Vec::new(),
        token_loot: String::new(),
        token_cost: 0,
    };
    if app.config.auto_type_fields {
        def.fields = type_field_template(app.vanilla_catalog.as_ref(), def.type_);
    }
    add_item(app, def);
}

/// Remove the selected item from the working catalog and reindex.
/// Vanilla items removed here are recorded as deletions when the preset is saved, so they also drop out of the applied catalog.
fn delete_selected_item(app: &mut ResalinatedApp) {
    let Some(idx) = app.selected_item_idx else {
        return;
    };
    let removed_name = app
        .working_catalog
        .as_ref()
        .and_then(|c| c.loot_defs.get(idx))
        .map(|d| d.name.clone());
    if let Some(cat) = app.working_catalog.as_mut() {
        if idx >= cat.loot_defs.len() {
            return;
        }
        cat.loot_defs.remove(idx);
        cat.by_name.clear();
        for (i, d) in cat.loot_defs.iter().enumerate() {
            cat.by_name.insert(d.name.clone(), i);
        }
        cat.black_starstone_index = cat.loot_defs.iter().position(|d| d.name == "black_pearl");
        cat.gray_starstone_index = cat.loot_defs.iter().position(|d| d.name == "gray_pearl");
    }
    if let Some(name) = removed_name {
        app.loot_disabled.remove(&name);
        app.shop_additions.remove(&name);
        app.craft_additions.remove(&name);
    }
    app.selected_item_idx = None;
}

/// Clone the selected item under a new unique "<name>_copy" name (full structure preserved).
fn clone_selected_item(app: &mut ResalinatedApp) {
    // Clone every multi-selected item (or just the single selection when none).
    let sources: Vec<usize> = if app.selected_item_idxs.len() > 1 {
        let mut v: Vec<usize> = app.selected_item_idxs.iter().copied().collect();
        v.sort_unstable();
        v
    } else {
        app.selected_item_idx.into_iter().collect()
    };
    if sources.is_empty() {
        return;
    }

    // Clone the sources and compute fresh names while only borrowing the catalog immutably.
    let new_defs: Vec<LootDef> = {
        let Some(cat) = app.working_catalog.as_ref() else {
            return;
        };
        sources
            .iter()
            .filter_map(|&idx| {
                let src = cat.loot_defs.get(idx)?;
                let mut def = src.clone();
                def.name = next_unique_name(cat, &format!("{}_copy", src.name));
                Some(def)
            })
            .collect()
    };
    for def in new_defs {
        add_item(app, def);
    }
}

/// One multiplier row (1.0 = vanilla) for magic cost/cooldown, colored when non-default, with a reset-to-1.0 button.
fn magic_mul_row(ui: &mut Ui, label: &str, value: &mut f32, speed: f32) {
    let changed = (*value - 1.0).abs() > 0.001;
    let label_rich = if changed {
        egui::RichText::new(label).color(CHANGED_COLOR)
    } else {
        egui::RichText::new(label)
    };
    ui.horizontal(|ui| {
        ui.label(label_rich);
        ui.add(egui::DragValue::new(value).speed(speed).range(0.0..=100.0))
            .on_hover_text("Multiplier vs vanilla: 1.0 = unchanged, 0.5 = half, 2.0 = double. (0 is treated as unchanged, use a small value like 0.01 for near-free.)");
        if changed && ui.button("↺").clicked() {
            *value = 1.0;
        }
    });
}

/// A default (zeroed) value of the same variant.
fn default_field_value(v: &LootFieldValue) -> LootFieldValue {
    match v {
        LootFieldValue::Float(_) => LootFieldValue::Float(0.0),
        LootFieldValue::Int(_) => LootFieldValue::Int(0),
        LootFieldValue::Bool(_) => LootFieldValue::Bool(false),
        LootFieldValue::String(_) => LootFieldValue::String(String::new()),
    }
}

/// The canonical field set for a loot type, taken from the vanilla item of that type with the most fields.
/// Values are reset to defaults, ids and data types are preserved.
fn type_field_template(vanilla: Option<&LootCatalog>, type_: i32) -> Vec<LootField> {
    let Some(v) = vanilla else {
        return Vec::new();
    };
    v.loot_defs
        .iter()
        .filter(|d| d.type_ == type_)
        .max_by_key(|d| d.fields.len())
        .map(|t| {
            t.fields
                .iter()
                .map(|f| LootField {
                    id: f.id,
                    data_type: f.data_type,
                    value: default_field_value(&f.value),
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Replace `def.fields` with the type template, preserving values of fields that already exist
/// (matched by id and value variant).
fn apply_type_template(def: &mut LootDef, template: Vec<LootField>) {
    if template.is_empty() {
        return;
    }
    let old = std::mem::take(&mut def.fields);
    def.fields = template
        .into_iter()
        .map(|mut tf| {
            if let Some(of) = old.iter().find(|o| {
                o.id == tf.id
                    && std::mem::discriminant(&o.value) == std::mem::discriminant(&tf.value)
            }) {
                tf.value = of.value.clone();
            }
            tf
        })
        .collect();
}

/// Replace the item at `idx`'s fields and flags with those of `src_name` (its "logic").
fn apply_copy_logic(app: &mut ResalinatedApp, idx: usize, src_name: &str) {
    let src = app
        .working_catalog
        .as_ref()
        .and_then(|c| c.loot_defs.iter().find(|d| d.name == src_name))
        .map(|d| (d.fields.clone(), d.flags.clone()));
    if let Some((fields, flags)) = src {
        if let Some(cat) = app.working_catalog.as_mut() {
            if let Some(def) = cat.loot_defs.get_mut(idx) {
                def.fields = fields;
                def.flags = flags;
            }
        }
    }
}

/// The magic slot field ids on weapons (X / Y / B).
const MAGIC_SLOT_IDS: [i32; 3] = [14, 15, 16];

/// Copy the magic state (slot fields + per-slot multipliers) of `def` into the magic clipboard.
fn copy_magic_from_def(
    def: &LootDef,
    overrides: &HashMap<String, HashMap<i32, MagicSlotOverrides>>,
    clipboard: &mut crate::magic_slot::MagicClipboard,
) {
    let mut fields: Vec<LootField> = Vec::new();
    for f in &def.fields {
        if MAGIC_SLOT_IDS.contains(&f.id) {
            fields.push(f.clone());
        }
    }
    let slot_overrides = overrides.get(&def.name).cloned().unwrap_or_default();
    let source = def
        .title
        .first()
        .filter(|t| !t.is_empty())
        .cloned()
        .unwrap_or_else(|| def.name.clone());
    *clipboard = crate::magic_slot::MagicClipboard {
        fields,
        overrides: slot_overrides,
        source: Some(source),
    };
}

/// Apply the magic clipboard to `def`: replaces the magic slot fields and the per-slot multipliers.
fn paste_magic_to_def(
    def: &mut LootDef,
    clip: &crate::magic_slot::MagicClipboard,
    overrides: &mut HashMap<String, HashMap<i32, MagicSlotOverrides>>,
) {
    let weapon_name = def.name.clone();
    for f in &mut def.fields {
        if MAGIC_SLOT_IDS.contains(&f.id) {
            if let Some(src) = clip.fields.iter().find(|sf| sf.id == f.id) {
                f.value = src.value.clone();
            }
        }
    }
    let slot_overrides = overrides.entry(weapon_name).or_default();
    for slot_id in MAGIC_SLOT_IDS {
        if let Some(src) = clip.overrides.get(&slot_id) {
            slot_overrides.insert(slot_id, src.clone());
        }
    }
}

/// Reset all magic on `def` to vanilla: slot fields get the vanilla values and the per-slot multipliers are cleared.
fn default_magic_on_def(
    def: &mut LootDef,
    vanilla: Option<&LootDef>,
    overrides: &mut HashMap<String, HashMap<i32, MagicSlotOverrides>>,
) {
    let weapon_name = def.name.clone();
    let vanilla_fields: Vec<LootField> = vanilla
        .map(|vd| {
            vd.fields
                .iter()
                .filter(|f| MAGIC_SLOT_IDS.contains(&f.id))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    for f in &mut def.fields {
        if MAGIC_SLOT_IDS.contains(&f.id) {
            if let Some(vf) = vanilla_fields.iter().find(|vf| vf.id == f.id) {
                f.value = vf.value.clone();
            }
        }
    }
    overrides.remove(&weapon_name);
}

/// Copy the magic state of the item at `src_idx` into the app's magic clipboard.
fn copy_magic_from(app: &mut ResalinatedApp, src_idx: usize) {
    let Some(cat) = app.working_catalog.as_ref() else {
        return;
    };
    let Some(src) = cat.loot_defs.get(src_idx) else {
        return;
    };
    copy_magic_from_def(src, &app.magic_slot_overrides, &mut app.magic_clipboard);
}

/// Apply the magic clipboard to the item at `idx`.
fn paste_magic_to(app: &mut ResalinatedApp, idx: usize) {
    let clip = app.magic_clipboard.clone();
    let Some(cat) = app.working_catalog.as_mut() else {
        return;
    };
    let Some(def) = cat.loot_defs.get_mut(idx) else {
        return;
    };
    paste_magic_to_def(def, &clip, &mut app.magic_slot_overrides);
}

/// Copy the flags of `def` into the flags clipboard.
fn copy_flags_from_def(def: &LootDef, clipboard: &mut Option<crate::magic_slot::FlagsClipboard>) {
    let source = def
        .title
        .first()
        .filter(|t| !t.is_empty())
        .cloned()
        .unwrap_or_else(|| def.name.clone());
    *clipboard = Some(crate::magic_slot::FlagsClipboard {
        flags: def.flags.clone(),
        source: Some(source),
    });
}

/// Apply the flags clipboard to `def`.
fn paste_flags_to_def(def: &mut LootDef, clip: &crate::magic_slot::FlagsClipboard) {
    def.flags = clip.flags.clone();
}

/// Reset all flags on `def` to vanilla.
fn default_flags_on_def(def: &mut LootDef, vanilla: Option<&LootDef>) {
    def.flags = vanilla.map(|v| v.flags.clone()).unwrap_or_default();
}

/// Copy the flags of the item at `src_idx` into the flags clipboard.
fn copy_flags_from(app: &mut ResalinatedApp, src_idx: usize) {
    let Some(cat) = app.working_catalog.as_ref() else {
        return;
    };
    let Some(src) = cat.loot_defs.get(src_idx) else {
        return;
    };
    copy_flags_from_def(src, &mut app.flags_clipboard);
}

/// Apply the flags clipboard to the item at `idx`.
fn paste_flags_to(app: &mut ResalinatedApp, idx: usize) {
    let Some(clip) = app.flags_clipboard.clone() else {
        return;
    };
    let Some(cat) = app.working_catalog.as_mut() else {
        return;
    };
    let Some(def) = cat.loot_defs.get_mut(idx) else {
        return;
    };
    paste_flags_to_def(def, &clip);
}

/// 'magic_items': (internal_name, display_title) pairs for all magic-type items in the catalog.
/// 'copy_candidates': (name, display, type, subtype) for "Copy logic from".
fn show_lootdef_editor(
    app: &mut ResalinatedApp,
    ui: &mut Ui,
    magic_items: &[(String, String)],
    copy_candidates: &[(String, String, i32, i32)],
    icon_capacity: i32,
    icon_choices: &[(i32, String)],
) {
    // Get the selected item index
    let Some(idx) = app.selected_item_idx else {
        ui.label("Select an item to edit.");
        return;
    };

    // Get working catalog and mutable def
    let working_catalog = match app.working_catalog.as_mut() {
        Some(c) => c,
        None => {
            ui.label("No working catalog.");
            return;
        }
    };
    let def = match working_catalog.loot_defs.get_mut(idx) {
        Some(d) => d,
        None => {
            ui.label("Invalid selection.");
            return;
        }
    };

    // Get vanilla def for comparison
    let vanilla = app
        .vanilla_catalog
        .as_ref()
        .and_then(|vc| vc.by_name.get(&def.name).map(|&i| &vc.loot_defs[i]))
        .cloned();

    // The type of the selected item, used by the copy-logic picker (rendered after the editor).
    let selected_type = def.type_;
    let selected_sub = def.sub_type;
    let selected_name = def.name.clone();

    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.style_mut().override_text_style = Some(egui::TextStyle::Body);
            ui.style_mut().text_styles.insert(
                egui::TextStyle::Body,
                egui::FontId::proportional(app.config.sidebar_font_size),
            );
            ui.heading("Loot Definition");

            if let Some(vanilla_def) = &vanilla {
                if ui.button("Reset Item to Vanilla").clicked() {
                    *def = vanilla_def.clone();
                    return;
                }
            }

            ui.separator();

            field_row(
                ui,
                "Name:",
                vanilla.as_ref().map(|v| def.name != v.name).unwrap_or(true),
                |ui| {
                    ui.text_edit_singleline(&mut def.name);
                    if let Some(v) = &vanilla {
                        if def.name != v.name && ui.button("↺").clicked() {
                            def.name = v.name.clone();
                        }
                    }
                },
            );

            field_row(
                ui,
                "Title:",
                vanilla
                    .as_ref()
                    .map(|v| def.title[0] != v.title[0])
                    .unwrap_or(true),
                |ui| {
                    ui.text_edit_singleline(&mut def.title[0]);
                    if let Some(v) = &vanilla {
                        if def.title[0] != v.title[0] && ui.button("↺").clicked() {
                            def.title[0] = v.title[0].clone();
                        }
                    }
                },
            );

            field_row(
                ui,
                "Description:",
                vanilla
                    .as_ref()
                    .map(|v| def.description[0] != v.description[0])
                    .unwrap_or(true),
                |ui| {
                    ui.text_edit_singleline(&mut def.description[0]);
                    if let Some(v) = &vanilla {
                        if def.description[0] != v.description[0] && ui.button("↺").clicked() {
                            def.description[0] = v.description[0].clone();
                        }
                    }
                },
            );

            // Type (editable dropdown) and subtype.
            let type_before = def.type_;
            field_row(
                ui,
                "Type:",
                vanilla
                    .as_ref()
                    .map(|v| def.type_ != v.type_)
                    .unwrap_or(true),
                |ui| {
                    egui::ComboBox::from_id_salt("item_type")
                        .selected_text(format!(
                            "{} ({})",
                            def.type_,
                            loot_names::get_type_name(def.type_)
                        ))
                        .show_ui(ui, |ui| {
                            for t in 0..=8 {
                                ui.selectable_value(
                                    &mut def.type_,
                                    t,
                                    format!("{} {}", t, loot_names::get_type_name(t)),
                                );
                            }
                        });
                    if let Some(v) = &vanilla {
                        if def.type_ != v.type_ && ui.button("↺").clicked() {
                            def.type_ = v.type_;
                        }
                    }
                },
            );
            // When the type changes and auto-fill is on, adopt that type's field set.
            if def.type_ != type_before && app.config.auto_type_fields {
                let template = type_field_template(app.vanilla_catalog.as_ref(), def.type_);
                apply_type_template(def, template);
            }
            field_row(
                ui,
                "Subtype:",
                vanilla
                    .as_ref()
                    .map(|v| def.sub_type != v.sub_type)
                    .unwrap_or(true),
                |ui| {
                    ui.add(egui::DragValue::new(&mut def.sub_type));
                    ui.label(loot_names::get_subtype_name(def.type_, def.sub_type));
                    if let Some(v) = &vanilla {
                        if def.sub_type != v.sub_type && ui.button("↺").clicked() {
                            def.sub_type = v.sub_type;
                        }
                    }
                },
            );

            // Craft / equipment menu: independent of Shops. Adds this item to crafting menus.
            ui.collapsing("Craft / Equipment menu", |ui| {
                let name = def.name.clone();
                let mut craftable = app.craft_additions.contains_key(&name);
                if ui
                    .checkbox(&mut craftable, "Add to craft / equipment menu")
                    .changed()
                {
                    if craftable {
                        app.craft_additions.entry(name.clone()).or_default();
                    } else {
                        app.craft_additions.remove(&name);
                    }
                }
                if let Some(material) = app.craft_additions.get_mut(&name) {
                    ui.horizontal(|ui| {
                        ui.label("Craft material (optional):");
                        ui.text_edit_singleline(material);
                    });
                    ui.label(
                        egui::RichText::new(
                            "If the item already has a recipe it appears as-is. \
                            Otherwise set a material (an item's internal name) to craft it from. \
                            Applies after Apply Changes.",
                        )
                        ,
                    );
                }
            });

            field_row(
                ui,
                "Cost:",
                vanilla
                    .as_ref()
                    .map(|v| (def.cost - v.cost).abs() > 0.001)
                    .unwrap_or(true),
                |ui| {
                    let mut cost = def.cost;
                    if ui.add(egui::DragValue::new(&mut cost).speed(1.0)).changed() {
                        def.cost = cost;
                    }
                    if let Some(v) = &vanilla {
                        if (cost - v.cost).abs() > 0.001 && ui.button("↺").clicked() {
                            def.cost = v.cost;
                        }
                    }
                },
            );

            field_row(
                ui,
                "Token Cost:",
                vanilla
                    .as_ref()
                    .map(|v| def.token_cost != v.token_cost)
                    .unwrap_or(true),
                |ui| {
                    ui.add(egui::DragValue::new(&mut def.token_cost));
                    ui.label(
                        egui::RichText::new(
                            "When > 0, the item costs this many tokens instead of silver.",
                        )
                        ,
                    );
                    if let Some(v) = &vanilla {
                        if def.token_cost != v.token_cost && ui.button("↺").clicked() {
                            def.token_cost = v.token_cost;
                        }
                    }
                },
            );

            field_row(
                ui,
                "Img:",
                vanilla.as_ref().map(|v| def.img != v.img).unwrap_or(true),
                |ui| {
                    ui.add(egui::DragValue::new(&mut def.img));
                    // Indicate whether this is a vanilla or custom-atlas icon.
                    if icon_capacity > 0 && def.img >= icon_capacity {
                        ui.label(
                            egui::RichText::new(format!("custom #{}", def.img - icon_capacity))
                                .color(egui::Color32::LIGHT_GREEN),
                        );
                    }
                    if ui
                        .button("Pick img")
                        .on_hover_text("Choose a vanilla game icon (searchable)")
                        .clicked()
                    {
                        app.vanilla_icon_picker_open = true;
                        app.vanilla_icon_search.clear();
                        app.vanilla_icon_picker_focus = true;
                    }
                    if ui
                        .button("Pick custom img")
                        .on_hover_text("Choose a custom icon (searchable)")
                        .clicked()
                    {
                        app.custom_icon_picker_open = true;
                        app.custom_icon_search.clear();
                        app.custom_icon_picker_focus = true;
                    }
                    if let Some(v) = &vanilla {
                        if def.img != v.img && ui.button("↺").clicked() {
                            def.img = v.img;
                        }
                    }
                },
            );

            field_row(
                ui,
                "AltImg:",
                vanilla
                    .as_ref()
                    .map(|v| def.alt_img != v.alt_img)
                    .unwrap_or(true),
                |ui| {
                    ui.add(egui::DragValue::new(&mut def.alt_img));
                    if let Some(v) = &vanilla {
                        if def.alt_img != v.alt_img && ui.button("↺").clicked() {
                            def.alt_img = v.alt_img;
                        }
                    }
                },
            );

            field_row(
                ui,
                "Texture:",
                vanilla
                    .as_ref()
                    .map(|v| def.texture != v.texture)
                    .unwrap_or(true),
                |ui| {
                    ui.text_edit_singleline(&mut def.texture);
                    if let Some(v) = &vanilla {
                        if def.texture != v.texture && ui.button("↺").clicked() {
                            def.texture = v.texture.clone();
                        }
                    }
                },
            );

            ui.separator();

            // Numeric / String Fields
            ui.collapsing(format!("Fields ({})", def.fields.len()), |ui| {
                // Magic helpers: only meaningful for weapons (type 1) with magic slots.
                let is_weapon = def.type_ == 1;
                let has_magic_slots = def.fields.iter().any(|f| MAGIC_SLOT_IDS.contains(&f.id));
                if is_weapon && has_magic_slots {
                    ui.horizontal(|ui| {
                        if ui
                            .button("Default magic")
                            .on_hover_text("Set all magic on this item back to vanilla")
                            .clicked()
                        {
                            default_magic_on_def(def, vanilla.as_ref(), &mut app.magic_slot_overrides);
                        }
                        if ui
                            .button("Copy magic (pick)")
                            .on_hover_text("Open the item picker, then copy the selected item's magic onto this item")
                            .clicked()
                        {
                            app.magic_copy_picker_open = true;
                            app.copy_picker_search.clear();
                            app.copy_picker_focus = true;
                        }
                        if ui
                            .button("Copy magic")
                            .on_hover_text("Copy this item's magic to the clipboard")
                            .clicked()
                        {
                            copy_magic_from_def(def, &app.magic_slot_overrides, &mut app.magic_clipboard);
                        }
                        let has_clip = !app.magic_clipboard.fields.is_empty()
                            || !app.magic_clipboard.overrides.is_empty();
                        let paste_resp = ui.add_enabled(has_clip, egui::Button::new("Paste magic"));
                        let paste_hover = if has_clip {
                            match &app.magic_clipboard.source {
                                Some(s) => format!("Apply the copied magic (from {}) to this item", s),
                                None => "Apply the copied magic to this item".to_string(),
                            }
                        } else {
                            "Copy magic first".to_string()
                        };
                        if paste_resp.on_hover_text(paste_hover).clicked() {
                            paste_magic_to_def(def, &app.magic_clipboard, &mut app.magic_slot_overrides);
                        }
                    });
                    ui.separator();
                }

                // Copy logic (fields + flags) from another item, to give a new item working logic.
                if ui
                    .button("Copy logic from...")
                    .on_hover_text(
                        "Replace this item's fields and flags with another item's (its logic)",
                    )
                    .clicked()
                {
                    app.copy_picker_open = true;
                    app.copy_picker_search.clear();
                    app.copy_picker_focus = true;
                }

                let mut remove_field: Option<usize> = None;
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        let weapon_name = def.name.clone();

                        for (field_index, field) in def.fields.iter_mut().enumerate() {
                            let field_name = loot_names::get_field_name(def.type_, field.id);
                            let changed = vanilla
                                .as_ref()
                                .and_then(|vdef| vdef.fields.iter().find(|vf| vf.id == field.id))
                                .map(|vf| values_differ(&field.value, &vf.value))
                                .unwrap_or(true);

                            let label = if changed {
                                egui::RichText::new(format!("{}: ", field_name))
                                    .color(CHANGED_COLOR)
                            } else {
                                egui::RichText::new(format!("{}: ", field_name))
                            };

                            // For weapon magic slots, use a popup window
                            let is_magic_slot = def.type_ == 1
                                && (field.id == 14 || field.id == 15 || field.id == 16);

                            if is_magic_slot {
                                let current = match &field.value {
                                    LootFieldValue::String(s) => s.clone(),
                                    _ => String::new(),
                                };
                                let vanilla_val = vanilla
                                    .as_ref()
                                    .and_then(|v| v.fields.iter().find(|vf| vf.id == field.id))
                                    .map(|vf| match &vf.value {
                                        LootFieldValue::String(s) => s.clone(),
                                        _ => String::new(),
                                    });

                                let current_display = if current.is_empty() {
                                    "- None -".to_string()
                                } else {
                                    magic_items
                                        .iter()
                                        .find(|(n, _)| n == &current)
                                        .map(|(_, t)| format!("{} ({})", t, current))
                                        .unwrap_or_else(|| format!("? {}", current))
                                };

                                // Button that opens the popup
                                ui.horizontal(|ui| {
                                    ui.label(label);
                                    if ui.button(&current_display).clicked() {
                                        app.magic_item_picker_open = true;
                                        app.magic_item_search.clear();
                                        app.magic_item_picker_focus = true;
                                        app.magic_item_picker_target_slot_id = Some(field.id);
                                    }
                                    if let Some(van) = &vanilla_val {
                                        if van != &current && ui.button("↺").clicked() {
                                            field.value = LootFieldValue::String(van.clone());
                                        }
                                    }
                                });

                                // Magic damage override after each magic slot field
                                let slot_id = field.id;
                                let weapon_overrides = app
                                    .magic_slot_overrides
                                    .entry(weapon_name.clone())
                                    .or_default();
                                let slot_override = weapon_overrides.entry(slot_id).or_default();

                                let dmg_label = match slot_id {
                                    14 => "Magic [X] Damage Multiplier:",
                                    15 => "Magic [Y] Damage Multiplier:",
                                    16 => "Magic [B] Damage Multiplier:",
                                    _ => "Magic Damage Multiplier:",
                                };

                                let changed = (slot_override.damage - 0.0f32).abs() > 0.001;
                                let label_rich = if changed {
                                    egui::RichText::new(dmg_label).color(CHANGED_COLOR)
                                } else {
                                    egui::RichText::new(dmg_label)
                                };

                                ui.horizontal(|ui| {
                                    ui.label(label_rich);
                                    ui.add(
                                        egui::DragValue::new(&mut slot_override.damage)
                                            .speed(app.config.drag_value_sensitivity),
                                    );
                                    if changed && ui.button("↺").clicked() {
                                        slot_override.damage = 0.0;
                                    }
                                });

                                // Cost (MP/Rage) and cooldown multipliers for the same slot.
                                let slot_letter = match slot_id {
                                    14 => "X",
                                    15 => "Y",
                                    16 => "B",
                                    _ => "?",
                                };
                                magic_mul_row(
                                    ui,
                                    &format!("Magic [{}] Cost Multiplier:", slot_letter),
                                    &mut slot_override.cost,
                                    app.config.drag_value_sensitivity,
                                );
                                magic_mul_row(
                                    ui,
                                    &format!("Magic [{}] Cooldown Multiplier:", slot_letter),
                                    &mut slot_override.cooldown,
                                    app.config.drag_value_sensitivity,
                                );
                            } else {
                                ui.horizontal(|ui| {
                                    ui.label(label);
                                    match &mut field.value {
                                        LootFieldValue::Float(v) => {
                                            ui.add(
                                                egui::DragValue::new(v)
                                                    .speed(app.config.drag_value_sensitivity),
                                            );
                                        }
                                        LootFieldValue::Int(v) => {
                                            ui.add(egui::DragValue::new(v));
                                        }
                                        LootFieldValue::Bool(v) => {
                                            ui.checkbox(v, "");
                                        }
                                        LootFieldValue::String(v) => {
                                            ui.text_edit_singleline(v);
                                        }
                                    }
                                    // Reset to vanilla
                                    if let Some(vanilla_def) = &vanilla {
                                        if let Some(vf) =
                                            vanilla_def.fields.iter().find(|vf| vf.id == field.id)
                                        {
                                            if values_differ(&field.value, &vf.value)
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
                        }
                    });

                if let Some(i) = remove_field {
                    if i < def.fields.len() {
                        def.fields.remove(i);
                    }
                }
            });

            // Flags
            let flag_count = loot_names::get_loot_flag_count(def.type_);
            egui::CollapsingHeader::new(format!(
                "Flags ({} active / {} total)",
                def.flags.len(),
                flag_count
            ))
            .id_salt(("item_flags", idx))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .button("Default flags")
                        .on_hover_text("Set all flags on this item back to vanilla")
                        .clicked()
                    {
                        default_flags_on_def(def, vanilla.as_ref());
                    }
                    if ui
                        .button("Copy flags (pick)")
                        .on_hover_text("Open the item picker, then copy the selected item's flags onto this item")
                        .clicked()
                    {
                        app.flags_copy_picker_open = true;
                        app.copy_picker_search.clear();
                        app.copy_picker_focus = true;
                    }
                    if ui
                        .button("Copy flags")
                        .on_hover_text("Copy this item's flags to the clipboard")
                        .clicked()
                    {
                        copy_flags_from_def(def, &mut app.flags_clipboard);
                    }
                    let has_clip = app.flags_clipboard.is_some();
                    let paste_resp = ui.add_enabled(has_clip, egui::Button::new("Paste flags"));
                    let paste_hover = if has_clip {
                        match &app.flags_clipboard.as_ref().unwrap().source {
                            Some(s) => format!("Apply the copied flags (from {}) to this item", s),
                            None => "Apply the copied flags to this item".to_string(),
                        }
                    } else {
                        "Copy flags first".to_string()
                    };
                    if paste_resp.on_hover_text(paste_hover).clicked() {
                        if let Some(clip) = &app.flags_clipboard {
                            paste_flags_to_def(def, clip);
                        }
                    }
                });
                ui.separator();
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .show(ui, |ui| {
                        show_flag_checkboxes(
                            ui,
                            &mut def.flags,
                            flag_count,
                            def.type_,
                            vanilla.as_ref().map(|v| &v.flags),
                            loot_names::get_flag_name,
                        );
                    });
                // For talismans (type 6), show the configured boost range next to each flag.
                if def.type_ == 6 {
                    ui.separator();
                    ui.label(
                        egui::RichText::new("Boost configs (Talisman Boosts tab)")
                            .strong(),
                    );
                    for f in &def.flags {
                        let name = loot_names::get_flag_name(6, *f);
                        let unit = crate::charm_boost::charm_boost_unit(*f);
                        let suffix = match unit {
                            crate::charm_boost::CharmBoostUnit::Percent => "%",
                            crate::charm_boost::CharmBoostUnit::Flat => "",
                        };
                        let range = app
                            .charm_boosts
                            .get(f)
                            .cloned()
                            .unwrap_or_else(|| crate::charm_boost::CharmBoostRange::vanilla(*f));
                        let changed = range.is_modified(*f);
                        ui.horizontal(|ui| {
                            if changed {
                                ui.colored_label(CHANGED_COLOR, name);
                            } else {
                                ui.label(name);
                            }
                            if range.static_boost {
                                ui.label(format!("static {:.1}{}", range.static_value, suffix));
                            } else if (range.max - range.min).abs() > 0.0001 {
                                ui.label(format!(
                                    "{:.1}{} - {:.1}{}",
                                    range.min, suffix, range.max, suffix
                                ));
                            } else {
                                ui.label(format!("{:.1}{}", range.min, suffix));
                            }
                        });
                    }
                }
            });
        });

    // "Copy logic from" picker: a searchable popup of same-type items (def borrow released here).
    if app.copy_picker_open {
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
                    "Items of type {} ({})",
                    selected_type,
                    loot_names::get_type_name(selected_type)
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
                        *t == selected_type
                            && n != &selected_name
                            && (needle.is_empty()
                                || n.to_lowercase().contains(&needle)
                                || disp.to_lowercase().contains(&needle))
                    })
                    .collect();
                // Same subtype first for relevance.
                matches.sort_by_key(|(_, _, _, st)| (*st != selected_sub) as i32);

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
                            let label = if *st == selected_sub {
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
            apply_copy_logic(app, idx, &src_name);
            app.copy_picker_open = false;
        }
    }

    // "Copy magic (pick)" picker: same list as copy logic, but copies only the magic state (slot fields + multipliers) instead of the full logic.
    if app.magic_copy_picker_open {
        let mut chosen: Option<String> = None;
        let mut open = app.magic_copy_picker_open;
        egui::Window::new("Copy magic from")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.set_width(320.0);
                ui.label(format!(
                    "Weapons of type {} ({})",
                    selected_type,
                    loot_names::get_type_name(selected_type)
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
                        *t == selected_type
                            && n != &selected_name
                            && (needle.is_empty()
                                || n.to_lowercase().contains(&needle)
                                || disp.to_lowercase().contains(&needle))
                    })
                    .collect();
                matches.sort_by_key(|(_, _, _, st)| (*st != selected_sub) as i32);

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
                            let label = if *st == selected_sub {
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
        app.magic_copy_picker_open = open;
        if let Some(src_name) = chosen {
            if let Some(src_idx) = app
                .working_catalog
                .as_ref()
                .and_then(|c| c.by_name.get(&src_name).copied())
            {
                copy_magic_from(app, src_idx as usize);
                paste_magic_to(app, idx);
            }
            app.magic_copy_picker_open = false;
        }
    }

    // "Copy flags (pick)" picker: same list as copy logic, but copies only the flags instead of the full logic.
    if app.flags_copy_picker_open {
        let mut chosen: Option<String> = None;
        let mut open = app.flags_copy_picker_open;
        egui::Window::new("Copy flags from")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.set_width(320.0);
                ui.label(format!(
                    "Items of type {} ({})",
                    selected_type,
                    loot_names::get_type_name(selected_type)
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
                        *t == selected_type
                            && n != &selected_name
                            && (needle.is_empty()
                                || n.to_lowercase().contains(&needle)
                                || disp.to_lowercase().contains(&needle))
                    })
                    .collect();
                matches.sort_by_key(|(_, _, _, st)| (*st != selected_sub) as i32);

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
                            let label = if *st == selected_sub {
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
        app.flags_copy_picker_open = open;
        if let Some(src_name) = chosen {
            if let Some(src_idx) = app
                .working_catalog
                .as_ref()
                .and_then(|c| c.by_name.get(&src_name).copied())
            {
                copy_flags_from(app, src_idx as usize);
                paste_flags_to(app, idx);
            }
            app.flags_copy_picker_open = false;
        }
    }

    // Vanilla icon picker: searchable grid of the game's item icons.
    if app.vanilla_icon_picker_open {
        let mut open = app.vanilla_icon_picker_open;
        let mut chosen_img: Option<i32> = None;
        egui::Window::new("Pick game icon")
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.set_width(440.0);
                ui.horizontal(|ui| {
                    ui.label("🔍");
                    let resp = ui.text_edit_singleline(&mut app.vanilla_icon_search);
                    if app.vanilla_icon_picker_focus {
                        resp.request_focus();
                        app.vanilla_icon_picker_focus = false;
                    }
                });
                ui.separator();
                let needle = app.vanilla_icon_search.to_lowercase();
                let atlas = app.item_atlas.as_ref();
                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .show_viewport(ui, |ui, viewport| {
                        // Only rows whose y-range intersects the visible viewport are laid out each frame.
                        // Culled cells still advance the grid cursor via allocate_space with the exact cell size, so positions, row heights and the scrollbar stay exact while the widget count stays proportional to the viewport.
                        let icon_size = 64.0;
                        let label_h =
                            ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(10.0)));
                        let spacing_y = ui.spacing().item_spacing.y;
                        let pad_x = 2.0 * ui.spacing().button_padding.x;
                        let pad_y = 2.0 * ui.spacing().button_padding.y;
                        let overscan = 3.0 * (icon_size + pad_x);
                        let vp_min = viewport.min.y - overscan;
                        let vp_max = viewport.max.y + overscan;

                        egui::Grid::new("vanilla_icon_pick")
                            .spacing([8.0, 8.0])
                            .show(ui, |ui| {
                                let mut col = 0;
                                let mut row_y = 0.0f32;
                                let mut row_h = 0.0f32;
                                for (img, label) in icon_choices.iter().filter(|(img, label)| {
                                    needle.is_empty()
                                        || label.to_lowercase().contains(&needle)
                                        || img.to_string().contains(&needle)
                                }) {
                                    let word_count = label.split_whitespace().count();
                                    let item_w = icon_size + pad_x;
                                    let item_h = icon_size
                                        + pad_y
                                        + word_count as f32 * (label_h + spacing_y);
                                    row_h = row_h.max(item_h);
                                    let row_visible = row_y + row_h >= vp_min && row_y <= vp_max;
                                    if !row_visible {
                                        ui.allocate_space(egui::vec2(item_w, item_h));
                                    } else {
                                        ui.vertical(|ui| {
                                            let uv = atlas.and_then(|a| a.icon_uv_for_img(*img));
                                            let clicked = if let (Some(a), Some(uv)) = (atlas, uv) {
                                                ui.add(egui::Button::image(
                                                    egui::Image::from_texture(&a.texture)
                                                        .fit_to_exact_size(egui::vec2(
                                                            icon_size, icon_size,
                                                        ))
                                                        .uv(uv),
                                                ))
                                                .on_hover_text(format!("{} (img {})", label, img))
                                                .clicked()
                                            } else {
                                                ui.button(format!("img {}", img)).clicked()
                                            };
                                            if clicked {
                                                chosen_img = Some(*img);
                                            }
                                            add_item_label(ui, label, 10.0, false);
                                        });
                                    }
                                    col += 1;
                                    if col % 5 == 0 {
                                        ui.end_row();
                                        row_y += row_h + spacing_y;
                                        row_h = 0.0;
                                        col = 0;
                                    }
                                }
                            });
                    });
            });
        app.vanilla_icon_picker_open = open;
        if let Some(img) = chosen_img {
            if let Some(cat) = app.working_catalog.as_mut() {
                if let Some(def) = cat.loot_defs.get_mut(idx) {
                    def.img = img;
                }
            }
            app.vanilla_icon_picker_open = false;
        }
    }

    // Custom-icon picker: searchable grid of custom icons; choosing sets img = capacity + local.
    if app.custom_icon_picker_open {
        let mut open = app.custom_icon_picker_open;
        let mut chosen_img: Option<i32> = None;
        egui::Window::new("Pick custom icon")
            .collapsible(false)
            .resizable(true)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                ui.set_width(360.0);
                ui.horizontal(|ui| {
                    ui.label("🔍");
                    let resp = ui.text_edit_singleline(&mut app.custom_icon_search);
                    if app.custom_icon_picker_focus {
                        resp.request_focus();
                        app.custom_icon_picker_focus = false;
                    }
                });
                ui.label(egui::RichText::new(
                    "Search by the Img value. Custom icons are managed in the Images tab.",
                ));
                ui.separator();
                if app.image_editor.icons.is_empty() {
                    ui.label("No custom icons yet. Add some in the Images tab.");
                } else {
                    let needle = app.custom_icon_search.to_lowercase();
                    egui::ScrollArea::vertical()
                        .max_height(400.0)
                        .show_viewport(ui, |ui, viewport| {
                            // Only rows whose y-range intersects the visible viewport are laid out each frame.
                            // Culled cells still advance the grid cursor via allocate_space with the exact cell size, so positions, row heights and the scrollbar stay exact while the widget count stays proportional to the viewport.
                            let icon_size = 72.0;
                            let label_h =
                                ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(12.0)));
                            let spacing_y = ui.spacing().item_spacing.y;
                            let pad_x = 2.0 * ui.spacing().button_padding.x;
                            let pad_y = 2.0 * ui.spacing().button_padding.y;
                            let overscan = 3.0 * (icon_size + pad_x);
                            let vp_min = viewport.min.y - overscan;
                            let vp_max = viewport.max.y + overscan;

                            egui::Grid::new("custom_icon_pick")
                                .spacing([8.0, 8.0])
                                .show(ui, |ui| {
                                    let mut col = 0;
                                    let mut row_y = 0.0f32;
                                    let mut row_h = 0.0f32;
                                    for (local, handle) in
                                        app.image_editor.icons.iter().filter(|(local, _)| {
                                            let global = icon_capacity + *local;
                                            needle.is_empty()
                                                || global.to_string().contains(&needle)
                                        })
                                    {
                                        let global = icon_capacity + *local;
                                        let item_w = icon_size + pad_x;
                                        let item_h = icon_size + pad_y + label_h + spacing_y;
                                        row_h = row_h.max(item_h);
                                        let row_visible =
                                            row_y + row_h >= vp_min && row_y <= vp_max;
                                        if !row_visible {
                                            ui.allocate_space(egui::vec2(item_w, item_h));
                                        } else {
                                            ui.vertical(|ui| {
                                                if ui
                                                    .add(egui::Button::image(
                                                        egui::Image::from_texture(handle)
                                                            .fit_to_exact_size(egui::vec2(
                                                                icon_size, icon_size,
                                                            )),
                                                    ))
                                                    .on_hover_text(format!("Img = {}", global))
                                                    .clicked()
                                                {
                                                    chosen_img = Some(global);
                                                }
                                                ui.label(format!("{}", global));
                                            });
                                        }
                                        col += 1;
                                        if col % 4 == 0 {
                                            ui.end_row();
                                            row_y += row_h + spacing_y;
                                            row_h = 0.0;
                                            col = 0;
                                        }
                                    }
                                });
                        });
                }
            });
        app.custom_icon_picker_open = open;
        if let Some(global) = chosen_img {
            if let Some(cat) = app.working_catalog.as_mut() {
                if let Some(def) = cat.loot_defs.get_mut(idx) {
                    def.img = global;
                }
            }
            app.custom_icon_picker_open = false;
        }
    }

    if app.magic_item_picker_open {
        let target_slot_id = match app.magic_item_picker_target_slot_id {
            Some(id) => id,
            None => {
                // Shouldn't happen, but close safely
                app.magic_item_picker_open = false;
                return;
            }
        };

        egui::Window::new("Select Magic Item")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ui.ctx(), |ui| {
                ui.set_width(300.0);
                ui.horizontal(|ui| {
                    ui.label("🔍");
                    let resp = ui.text_edit_singleline(&mut app.magic_item_search);
                    if app.magic_item_picker_focus {
                        resp.request_focus();
                        app.magic_item_picker_focus = false;
                    }
                });
                ui.separator();

                let search_lower = app.magic_item_search.to_lowercase();
                let filtered: Vec<_> = magic_items
                    .iter()
                    .filter(|(name, title)| {
                        search_lower.is_empty()
                            || name.to_lowercase().contains(&search_lower)
                            || title.to_lowercase().contains(&search_lower)
                    })
                    .collect();

                // Virtualized rows: only the rows visible in the scroll viewport are laid out each frame.
                let row_height = ui
                    .text_style_height(&egui::TextStyle::Body)
                    .max(ui.spacing().interact_size.y);
                if filtered.is_empty() {
                    ui.label("No magic items found.");
                }
                egui::ScrollArea::vertical().max_height(400.0).show_rows(
                    ui,
                    row_height,
                    filtered.len(),
                    |ui, row_range| {
                        for i in row_range {
                            let (name, title) = filtered[i];
                            let entry_label = format!("{} ({})", title, name);
                            if ui
                                .add(egui::Button::selectable(false, entry_label).truncate())
                                .clicked()
                            {
                                // Update the correct magic slot field
                                if let Some(def) = app
                                    .working_catalog
                                    .as_mut()
                                    .and_then(|c| c.loot_defs.get_mut(app.selected_item_idx?))
                                {
                                    if let Some(field) =
                                        def.fields.iter_mut().find(|f| f.id == target_slot_id)
                                    {
                                        field.value = LootFieldValue::String(name.clone());
                                    }
                                }
                                app.magic_item_picker_open = false;
                                app.magic_item_picker_target_slot_id = None;
                            }
                        }
                    },
                );

                ui.separator();
                if ui.button("Cancel").clicked() {
                    app.magic_item_picker_open = false;
                    app.magic_item_picker_target_slot_id = None;
                }
            });
    }
}

/// Render the full flag list as checkboxes, one per defined flag index.
/// 'get_name_fn' maps (type_, flag_idx) -> &str.
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

fn values_differ(a: &LootFieldValue, b: &LootFieldValue) -> bool {
    match (a, b) {
        (LootFieldValue::Float(x), LootFieldValue::Float(y)) => (x - y).abs() > 0.001,
        (LootFieldValue::Int(x), LootFieldValue::Int(y)) => x != y,
        (LootFieldValue::Bool(x), LootFieldValue::Bool(y)) => x != y,
        (LootFieldValue::String(x), LootFieldValue::String(y)) => x != y,
        _ => true,
    }
}
