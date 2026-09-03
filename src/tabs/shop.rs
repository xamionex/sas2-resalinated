use crate::app::ResalinatedApp;
use crate::tabs::utils::CHANGED_COLOR;
use eframe::egui;
use egui::Ui;
use sas2_parser::dialog::DialogNode;
use std::collections::HashMap;

/// A store script entry: "item" or "flag:item".
fn parse_entry(entry: &str) -> (String, String) {
    if let Some((flag, item)) = entry.split_once(':') {
        (flag.to_string(), item.to_string())
    } else {
        (String::new(), entry.to_string())
    }
}

/// Split a store script into its entries (the game separates them with \r\n).
fn split_script(script: &str) -> Vec<String> {
    script
        .split('\n')
        .map(|l| l.trim_end_matches('\r').trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Rejoin entries into a store script.
fn join_script(entries: &[(String, String)]) -> String {
    entries
        .iter()
        .map(|(flag, item)| {
            if flag.is_empty() {
                item.clone()
            } else {
                format!("{}:{}", flag, item)
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

/// Command scripts (sell/salvage/split/reroll) are not item lists.
fn is_command_script(script: &str) -> bool {
    let first = script.lines().next().unwrap_or("").trim();
    matches!(first, "sell" | "salvage" | "split" | "reroll")
}

fn is_shop_node(node: &DialogNode) -> bool {
    !node.store_script.is_empty() && !is_command_script(&node.store_script)
}

enum ShopAction {
    AddItem {
        npc: usize,
        node: usize,
        item: String,
    },
    RemoveEntry {
        npc: usize,
        node: usize,
        entry: usize,
    },
    MoveEntry {
        npc: usize,
        node: usize,
        entry: usize,
        delta: i32,
    },
    SetFlag {
        npc: usize,
        node: usize,
        entry: usize,
        flag: String,
    },
    ResetNode {
        npc: usize,
        node: usize,
    },
    SetCost {
        item: String,
        cost: f32,
    },
    SetTokenCost {
        item: String,
        token_cost: i32,
    },
}

fn apply_actions(app: &mut ResalinatedApp, actions: Vec<ShopAction>) {
    let Some(dialog) = app.working_dialog.as_mut() else {
        return;
    };
    for action in actions {
        match action {
            ShopAction::AddItem { npc, node, item } => {
                if let Some(n) = dialog.npcs.get_mut(npc) {
                    if let Some(nd) = n.nodes.get_mut(node) {
                        let mut lines = split_script(&nd.store_script);
                        if !lines.contains(&item) {
                            lines.push(item);
                            let entries: Vec<(String, String)> =
                                lines.iter().map(|l| parse_entry(l)).collect();
                            nd.store_script = join_script(&entries);
                        }
                    }
                }
            }
            ShopAction::RemoveEntry { npc, node, entry } => {
                if let Some(n) = dialog.npcs.get_mut(npc) {
                    if let Some(nd) = n.nodes.get_mut(node) {
                        let mut entries: Vec<(String, String)> = split_script(&nd.store_script)
                            .iter()
                            .map(|l| parse_entry(l))
                            .collect();
                        if entry < entries.len() {
                            entries.remove(entry);
                            nd.store_script = join_script(&entries);
                        }
                    }
                }
            }
            ShopAction::MoveEntry {
                npc,
                node,
                entry,
                delta,
            } => {
                if let Some(n) = dialog.npcs.get_mut(npc) {
                    if let Some(nd) = n.nodes.get_mut(node) {
                        let mut entries: Vec<(String, String)> = split_script(&nd.store_script)
                            .iter()
                            .map(|l| parse_entry(l))
                            .collect();
                        let new_idx = entry as i32 + delta;
                        if new_idx >= 0 && new_idx < entries.len() as i32 {
                            let e = entries.remove(entry);
                            entries.insert(new_idx as usize, e);
                            nd.store_script = join_script(&entries);
                        }
                    }
                }
            }
            ShopAction::SetFlag {
                npc,
                node,
                entry,
                flag,
            } => {
                if let Some(n) = dialog.npcs.get_mut(npc) {
                    if let Some(nd) = n.nodes.get_mut(node) {
                        let mut entries: Vec<(String, String)> = split_script(&nd.store_script)
                            .iter()
                            .map(|l| parse_entry(l))
                            .collect();
                        if entry < entries.len() {
                            entries[entry].0 = flag;
                            nd.store_script = join_script(&entries);
                        }
                    }
                }
            }
            ShopAction::ResetNode { npc, node } => {
                if let Some(n) = dialog.npcs.get_mut(npc) {
                    if let Some(nd) = n.nodes.get_mut(node) {
                        if let Some(v) = app
                            .vanilla_dialog
                            .as_ref()
                            .and_then(|d| d.find_npc(&n.name))
                        {
                            if let Some(vn) = v.nodes.iter().find(|vn| vn.name == nd.name) {
                                nd.store_script = vn.store_script.clone();
                            }
                        }
                    }
                }
            }
            ShopAction::SetCost { item, cost } => {
                if let Some(cat) = app.working_catalog.as_mut() {
                    if let Some(def) = cat.loot_defs.iter_mut().find(|d| d.name == item) {
                        def.cost = cost;
                    }
                }
            }
            ShopAction::SetTokenCost { item, token_cost } => {
                if let Some(cat) = app.working_catalog.as_mut() {
                    if let Some(def) = cat.loot_defs.iter_mut().find(|d| d.name == item) {
                        def.token_cost = token_cost;
                    }
                }
            }
        }
    }
}

/// Draw one item in a grid cell: icon button + name + optional price line.
/// Returns the button response (clicked state is read by the caller).
fn draw_item_cell(
    ui: &mut Ui,
    app: &ResalinatedApp,
    name: &str,
    display: &str,
    cost: f32,
    token_cost: i32,
    icon_size: f32,
    selected: bool,
) -> egui::Response {
    let def = app
        .working_catalog
        .as_ref()
        .and_then(|c| c.by_name.get(name).map(|&i| &c.loot_defs[i]));
    let response = crate::tabs::items::draw_image_button(
        ui,
        app.item_atlas.as_ref(),
        def,
        icon_size,
        &app.image_editor,
    );
    let btn_w = response.rect.width();
    ui.set_max_width(btn_w);
    crate::tabs::items::add_item_label(ui, display, app.config.grid_font_size, selected);
    let price = if token_cost > 0 {
        format!("{} tokens", token_cost)
    } else {
        format!("{} silver", cost)
    };
    ui.label(egui::RichText::new(price));
    response
}

/// Render a searchable item grid (like the Items tab) with selectable cells.
/// Items are grouped by type-subtype category, exactly like the Items tab.
/// Selection is handled by the shared gesture helper (click / ctrl+click / shift+click / box).
/// `is_modified` decides whether an item counts as "modified" for the filter.
fn item_grid_selectable(
    ui: &mut Ui,
    app: &mut ResalinatedApp,
    search: &mut String,
    items: &[(String, String, f32, i32, i32, i32)],
    icon_size: f32,
    selected: &Option<String>,
    show_only_modified: bool,
    is_modified: impl Fn(&ResalinatedApp, &str) -> bool,
) {
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.text_edit_singleline(search);
        ui.checkbox(&mut app.shop_show_only_modified, "Show only modified");
        crate::tabs::multisel::mouse_help_button(ui, &[]);
    });
    let filter = search.to_lowercase();
    let filtered: Vec<&(String, String, f32, i32, i32, i32)> = items
        .iter()
        .filter(|(name, display, _, _, _, _)| {
            if show_only_modified && !is_modified(app, name) {
                return false;
            }
            filter.is_empty()
                || name.to_lowercase().contains(&filter)
                || display.to_lowercase().contains(&filter)
        })
        .collect();

    // Group by type-subtype category, like the Items tab.
    let mut grouped: std::collections::BTreeMap<
        String,
        Vec<&(String, String, f32, i32, i32, i32)>,
    > = std::collections::BTreeMap::new();
    for item in filtered {
        let cat = format!(
            "{} - {}",
            sas2_parser::loot_names::get_type_name(item.4),
            sas2_parser::loot_names::get_subtype_name(item.4, item.5)
        );
        grouped.entry(cat).or_default().push(item);
    }

    // Selection gesture state (click / ctrl+click / shift+click / shift+drag box).
    let mut gsel = std::mem::take(&mut app.shop_all_grid_sel);
    gsel.begin(ui);

    // Full display order (all filtered items, not just visible ones) so shift+click ranges work across scrolled-out items.
    gsel.display_order.clear();
    for (_, entries) in &grouped {
        for (name, _, _, _, _, _) in entries {
            gsel.display_order.push(name.clone());
        }
    }

    egui::ScrollArea::both()
        .scroll_source(crate::tabs::multisel::grid_scroll_source(ui))
        .auto_shrink([false; 2])
        .show_viewport(ui, |ui, viewport| {
            // Only items whose x-range intersects the visible viewport are laid out each frame.
            // Culled items still advance the grid cursor via allocate_space with the exact cell size, so positions, row heights and the scrollbar stay exact while the widget count stays proportional to the viewport.
            let label_h = ui.fonts_mut(|f| {
                f.row_height(&egui::FontId::proportional(app.config.grid_font_size))
            });
            let spacing_y = ui.spacing().item_spacing.y;
            let pad_x = 2.0 * ui.spacing().button_padding.x;
            let pad_y = 2.0 * ui.spacing().button_padding.y;
            let overscan = 3.0 * (icon_size + pad_x);
            let vp_min = viewport.min.x - overscan;
            let vp_max = viewport.max.x + overscan;

            for (cat, entries) in grouped {
                ui.style_mut().interaction.selectable_labels = false;
                ui.label(
                    egui::RichText::new(&cat)
                        .strong()
                        .size(app.config.category_font_size),
                );
                egui::Grid::new(("shop_item_grid", cat))
                    .spacing([8.0, 8.0])
                    .show(ui, |ui| {
                        let mut x = 0.0f32;
                        for (name, display, cost, token_cost, _, _) in entries {
                            let def = app.working_catalog.as_ref().and_then(|cat| {
                                cat.by_name.get(name).and_then(|&i| cat.loot_defs.get(i))
                            });
                            let has_icon = def
                                .and_then(|d| app.item_atlas.as_ref().and_then(|a| a.icon_uv(d)))
                                .is_some()
                                || def.is_some_and(|d| {
                                    let cap = app.image_editor.capacity as i32;
                                    cap > 0
                                        && d.img >= cap
                                        && app
                                            .image_editor
                                            .icons
                                            .iter()
                                            .any(|(local, _)| *local == d.img - cap)
                                });
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
                                let is_sel = selected.as_deref() == Some(name.as_str())
                                    || app.shop_all_selected_multi.contains(name.as_str())
                                    || gsel.is_box_hit(name);
                                let response = draw_item_cell(
                                    ui,
                                    app,
                                    name,
                                    display,
                                    *cost,
                                    *token_cost,
                                    icon_size,
                                    is_sel,
                                );
                                crate::tabs::multisel::paint_sel_outline(ui, response.rect, is_sel);
                                gsel.cell(response.rect, name.clone());
                            });
                        }
                    });
                ui.add_space(8.0);
            }

            gsel.update_target();
            gsel.paint(ui);
            gsel.end(
                ui,
                &mut app.shop_all_selected_multi,
                &mut app.shop_all_selected,
            );
            app.shop_all_grid_sel = gsel;
        });
}

pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    // Load the custom-icon atlas so custom item icons render in the grids.
    if let Some(gp) = app.game_path.clone() {
        app.image_editor.ensure_loaded(ui.ctx(), &gp);
    }

    let Some(dialog) = app.working_dialog.as_ref() else {
        ui.heading("Shop");
        ui.label("No dialog catalog loaded.");
        return;
    };
    let Some(cat) = app.working_catalog.as_ref() else {
        ui.heading("Shop");
        ui.label("No catalog loaded.");
        return;
    };

    // Item lookup: name -> (display, cost, token_cost, type, subtype).
    // Owned so the catalog borrow is released before any mutable app access.
    let mut item_info: HashMap<String, (String, f32, i32, i32, i32)> = HashMap::new();
    let mut all_items: Vec<(String, String, f32, i32, i32, i32)> = Vec::new();
    for d in &cat.loot_defs {
        let display = d
            .title
            .first()
            .filter(|t| !t.is_empty())
            .cloned()
            .unwrap_or_else(|| d.name.clone());
        item_info.insert(
            d.name.clone(),
            (display.clone(), d.cost, d.token_cost, d.type_, d.sub_type),
        );
        all_items.push((
            d.name.clone(),
            display,
            d.cost,
            d.token_cost,
            d.type_,
            d.sub_type,
        ));
    }
    all_items.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));

    // Merchants: NPCs with at least one shop node, with their total item count.
    let merchants: Vec<(usize, String, usize)> = dialog
        .npcs
        .iter()
        .enumerate()
        .filter_map(|(i, npc)| {
            let count: usize = npc
                .nodes
                .iter()
                .filter(|n| is_shop_node(n))
                .map(|n| split_script(&n.store_script).len())
                .sum();
            if count > 0 {
                Some((i, npc.name.clone(), count))
            } else {
                None
            }
        })
        .collect();

    // Location label for a merchant: "map (x, y)" when the game places it.
    let location_of =
        |locations: &HashMap<String, (String, f32, f32)>, name: &str| -> Option<String> {
            locations
                .get(name)
                .map(|(map, x, y)| format!("{} ({:.0}, {:.0})", map, x, y))
        };

    // Track the previously viewed merchant so the shelf gesture anchor can be reset when switching shops (range anchors are per-merchant).
    let prev_npc = app.shop_last_npc;
    app.shop_last_npc = if app.shop_all_shops {
        None
    } else {
        app.shop_selected_npc
    };
    if prev_npc != app.shop_last_npc {
        app.shop_shelf_grid_sel.anchor = None;
        app.shop_shelf_grid_sel.reset_gesture();
    }

    // Heading with the merchant picker inlined; the picker wraps to multiple lines so it never runs off-screen.
    ui.horizontal(|ui| {
        ui.heading("Shop");
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            if ui
                .selectable_label(app.shop_all_shops, "All Shops")
                .clicked()
            {
                app.shop_all_shops = true;
                app.shop_selected_npc = None;
                app.shop_selected_entry = None;
            }
            let filter = app.shop_merchant_search.to_lowercase();
            for (i, name, _count) in &merchants {
                if !filter.is_empty() && !name.to_lowercase().contains(&filter) {
                    continue;
                }
                if ui
                    .selectable_label(
                        !app.shop_all_shops && app.shop_selected_npc == Some(*i),
                        name,
                    )
                    .clicked()
                {
                    app.shop_selected_npc = Some(*i);
                    app.shop_all_shops = false;
                    app.shop_selected_entry = None;
                }
            }
            ui.label("Search:");
            ui.text_edit_singleline(&mut app.shop_merchant_search);
        });
    });
    ui.add(
        egui::Label::new(
            egui::RichText::new(
                "Edit each merchant's inventory (the store script in their dialog). \
                Buy price comes from the item's Cost field; currency is silver, or tokens when the item has a token cost. \
                An optional flag gates an entry behind progression (flag:item).",
            )
            ,
        )
        .wrap(),
    );
    ui.separator();

    let mut actions: Vec<ShopAction> = Vec::new();
    let mut picker_target: Option<(usize, usize)> = None;

    egui::CentralPanel::default()
        .frame(egui::Frame::central_panel(&ui.style()).inner_margin(2.0))
        .show_inside(ui, |ui| {
        if app.shop_all_shops {
            ui.heading("All Shops");
            ui.label(
                egui::RichText::new(
                    "Items selected here are appended to every merchant's buy menu. \
                    An optional flag gates the item behind progression.",
                )
                ,
            );

            // Right panel: edit the selected item (sell-in-all-shops toggle).
            let all_edit = egui::Panel::right("shop_all_edit")
                .resizable(true)
                .default_size(300.0)
                .min_size(220.0)
                .max_size(ui.available_width() * 0.5)
                .show_inside(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    // With a multi-selection active, hide the single-item header so it doesn't give the false impression that edits apply to one item.
                    let multi_count = app.shop_all_selected_multi.len();
                    let mut name = String::new();
                    let mut cost = 0.0f32;
                    let mut token_cost = 0i32;
                    if multi_count > 1 {
                        ui.heading("Edit Selected Items");
                        ui.label(format!("{} items selected", multi_count));
                        ui.separator();
                        // Seed the cost/token fields from the first selected item so the drag values show something meaningful instead of 0.
                        if let Some(first) = app.shop_all_selected_multi.iter().next() {
                            if let Some((_, c, tc, _, _)) = item_info.get(first) {
                                cost = *c;
                                token_cost = *tc;
                            }
                        }
                    } else {
                        ui.heading("Selected Item");
                        let Some(n) = app.shop_all_selected.clone() else {
                            ui.label("Click an item to edit it.");
                            return;
                        };
                        let Some((display, c, tc, _, _)) = item_info.get(&n) else {
                            ui.label("Invalid selection.");
                            return;
                        };
                        name = n;
                        cost = *c;
                        token_cost = *tc;
                        ui.label(egui::RichText::new(display).strong());
                        ui.label(format!("({})", name));
                        ui.separator();
                    }
                    // The sell toggle applies to every selected item, not just the single selection.
                    let sell_targets: Vec<String> = if multi_count > 1 {
                        app.shop_all_selected_multi.iter().cloned().collect()
                    } else {
                        vec![name.clone()]
                    };
                    let all_selling = sell_targets
                        .iter()
                        .all(|n| app.shop_additions.contains_key(n));
                    let mut sell = all_selling;
                    if ui
                        .checkbox(&mut sell, "Sell in all shops")
                        .on_hover_text(if multi_count > 1 {
                            "Append every right-click selected item to every merchant's buy menu"
                        } else {
                            "Append this item to every merchant's buy menu"
                        })
                        .changed()
                    {
                        for n in &sell_targets {
                            if sell {
                                app.shop_additions.entry(n.clone()).or_default();
                            } else {
                                app.shop_additions.remove(n);
                            }
                        }
                    }
                    if sell {
                        if let Some(flag) = app.shop_additions.get_mut(&name) {
                            let flag_changed = !flag.is_empty();
                            ui.horizontal(|ui| {
                                if flag_changed {
                                    ui.colored_label(CHANGED_COLOR, "Flag:");
                                } else {
                                    ui.label("Flag:");
                                }
                                ui.text_edit_singleline(flag).on_hover_text(
                                    "Optional flag required to see this item (empty = always for sale)",
                                );
                                if flag_changed && ui.button("↺").clicked() {
                                    flag.clear();
                                }
                            });
                        }
                    }
                    ui.separator();
                    // Vanilla cost for the modified indicator and reset.
                    let vanilla_c = app.vanilla_catalog.as_ref().and_then(|vc| {
                        vc.by_name.get(&name).map(|&i| &vc.loot_defs[i])
                    });
                    let cost_changed = vanilla_c
                        .map(|v| (v.cost - cost).abs() > 0.001)
                        .unwrap_or(true);
                    ui.horizontal(|ui| {
                        if cost_changed {
                            ui.colored_label(CHANGED_COLOR, "Cost (silver):");
                        } else {
                            ui.label("Cost (silver):");
                        }
                        let mut new_cost = cost;
                        if ui
                            .add(egui::DragValue::new(&mut new_cost).speed(1.0))
                            .changed()
                        {
                            // Apply to every selected item, not just the single selection.
                            for n in &sell_targets {
                                actions.push(ShopAction::SetCost {
                                    item: n.clone(),
                                    cost: new_cost,
                                });
                            }
                        }
                        if cost_changed {
                            if let Some(v) = vanilla_c {
                                if ui.button("↺").clicked() {
                                    for n in &sell_targets {
                                        actions.push(ShopAction::SetCost {
                                            item: n.clone(),
                                            cost: v.cost,
                                        });
                                    }
                                }
                            }
                        }
                    });
                    let token_changed = vanilla_c
                        .map(|v| v.token_cost != token_cost)
                        .unwrap_or(true);
                    ui.horizontal(|ui| {
                        if token_changed {
                            ui.colored_label(CHANGED_COLOR, "Token cost:");
                        } else {
                            ui.label("Token cost:");
                        }
                        let mut new_token = token_cost;
                        if ui.add(egui::DragValue::new(&mut new_token)).changed() {
                            for n in &sell_targets {
                                actions.push(ShopAction::SetTokenCost {
                                    item: n.clone(),
                                    token_cost: new_token,
                                });
                            }
                        }
                        if token_changed {
                            if let Some(v) = vanilla_c {
                                if ui.button("↺").clicked() {
                                    for n in &sell_targets {
                                        actions.push(ShopAction::SetTokenCost {
                                            item: n.clone(),
                                            token_cost: v.token_cost,
                                        });
                                    }
                                }
                            }
                        }
                    });
                    ui.label(
                        egui::RichText::new(
                            "Cost changes apply to the item everywhere (same as the Items tab).",
                        )
                        ,
                    );
                });

            // Central: the selectable item grid.
            egui::CentralPanel::default()
                .frame(egui::Frame::central_panel(&ui.style()).inner_margin(2.0))
                .show_inside(ui, |ui| {
                    // Multi-select toolbar for the All Shops grid, above the grid so it is always visible.
                    let multi_count = app.shop_all_selected_multi.len();
                    if multi_count > 1 {
                        ui.horizontal(|ui| {
                            if ui
                                .button(format!("Remove selected ({})", multi_count))
                                .on_hover_text("Stop selling the right-click selected items in all shops")
                                .clicked()
                            {
                                for name in app.shop_all_selected_multi.iter() {
                                    app.shop_additions.remove(name);
                                }
                                app.shop_all_selected_multi.clear();
                                app.shop_all_selected = None;
                            }
                        });
                    }
                    let mut search = app.shop_all_search.clone();
                    let selected = app.shop_all_selected.clone();
                    let show_only = app.shop_show_only_modified;
                    item_grid_selectable(
                        ui,
                        app,
                        &mut search,
                        &all_items,
                        app.config.item_icon_size,
                        &selected,
                        show_only,
                        |app, name| app.shop_additions.contains_key(name),
                    );
                    app.shop_all_search = search;
                });
            let _ = all_edit;
            return;
        }

        let Some(npc_idx) = app.shop_selected_npc else {
            ui.label("Select a merchant to edit their inventory.");
            return;
        };
        let Some(dialog) = app.working_dialog.as_ref() else {
            return;
        };
        let Some(npc) = dialog.npcs.get(npc_idx) else {
            ui.label("Invalid selection.");
            return;
        };
        let npc_name = npc.name.clone();
        // Owned copy of the NPC so the panel closures don't borrow `app`.
        let npc_owned = npc.clone();
        // Owned vanilla store scripts for this NPC (for the "modified" filter).
        let vanilla_scripts: Vec<Vec<(String, String)>> = app
            .vanilla_dialog
            .as_ref()
            .and_then(|d| d.find_npc(&npc_name))
            .map(|v| {
                v.nodes
                    .iter()
                    .map(|n| {
                        split_script(&n.store_script)
                            .iter()
                            .map(|l| parse_entry(l))
                            .collect()
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Right panel: edit the selected shelf entry (like the Items tab's details panel).
        let edit_panel = egui::Panel::right("shop_entry_edit")
            .resizable(true)
            .default_size(300.0)
            .min_size(220.0)
            .max_size(ui.available_width() * 0.5)
            .show_inside(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // Multi-selection: remove every selected entry from this merchant.
                let multi_entries: Vec<(usize, usize)> = app
                    .shop_selected_entries_multi
                    .get(&npc_idx)
                    .map(|s| s.iter().copied().collect())
                    .unwrap_or_default();
                if multi_entries.len() > 1 {
                    ui.heading("Edit Selected Entries");
                    ui.label(format!("{} entries selected", multi_entries.len()));
                    ui.add_space(4.0);

                    // Cost / Token cost apply to every selected entry's item.
                    // Seed the fields from the first selected entry.
                    let first_item = multi_entries
                        .first()
                        .and_then(|(n, e)| {
                            npc_owned
                                .nodes
                                .get(*n)
                                .and_then(|nd| {
                                    split_script(&nd.store_script)
                                        .get(*e)
                                        .map(|l| parse_entry(l).1)
                                })
                        })
                        .unwrap_or_default();
                    let first_cost = item_info
                        .get(&first_item)
                        .map(|(_, c, _, _, _)| *c)
                        .unwrap_or(0.0);
                    let first_token = item_info
                        .get(&first_item)
                        .map(|(_, _, tc, _, _)| *tc)
                        .unwrap_or(0);

                    let mut cost = first_cost;
                    let mut token_cost = first_token;
                    let cost_changed = ui
                        .horizontal(|ui| {
                            ui.label("Cost (silver):");
                            ui.add(egui::DragValue::new(&mut cost).speed(1.0)).changed()
                        })
                        .inner;
                    let token_changed = ui
                        .horizontal(|ui| {
                            ui.label("Token cost:");
                            ui.add(egui::DragValue::new(&mut token_cost)).changed()
                        })
                        .inner;
                    if cost_changed || token_changed {
                        // Collect the unique item names among the selected entries.
                        let mut items: Vec<String> = Vec::new();
                        for (n, e) in &multi_entries {
                            if let Some(nd) = npc_owned.nodes.get(*n) {
                                if let Some(l) = split_script(&nd.store_script).get(*e) {
                                    let item = parse_entry(l).1;
                                    if !items.contains(&item) {
                                        items.push(item);
                                    }
                                }
                            }
                        }
                        for item in items {
                            if cost_changed {
                                actions.push(ShopAction::SetCost {
                                    item: item.clone(),
                                    cost,
                                });
                            }
                            if token_changed {
                                actions.push(ShopAction::SetTokenCost {
                                    item,
                                    token_cost,
                                });
                            }
                        }
                    }
                    ui.add_space(4.0);
                    if ui
                        .button("Remove all selected")
                        .on_hover_text("Remove the selected entries from this merchant")
                        .clicked()
                    {
                        for (n, e) in &multi_entries {
                            actions.push(ShopAction::RemoveEntry {
                                npc: npc_idx,
                                node: *n,
                                entry: *e,
                            });
                        }
                        if let Some(sel) = app.shop_selected_entries_multi.get_mut(&npc_idx) {
                            sel.clear();
                        }
                        app.shop_selected_entry = None;
                    }
                    ui.separator();
                    // Hide the single-entry editor while a multi-selection is active.
                    return;
                }

                ui.heading("Selected Item");
                let Some((sel_node, sel_entry)) = app.shop_selected_entry else {
                    ui.label("Click an item on the shelf to edit it.");
                    return;
                };
                let Some(node) = npc_owned.nodes.get(sel_node) else {
                    ui.label("Invalid selection.");
                    return;
                };
                if !is_shop_node(node) {
                    ui.label("Invalid selection.");
                    return;
                }
                let entries: Vec<(String, String)> =
                    split_script(&node.store_script).iter().map(|l| parse_entry(l)).collect();
                if sel_entry >= entries.len() {
                    ui.label("Invalid selection.");
                    return;
                }
                let (_, item) = &entries[sel_entry];
                let mut flag = entries[sel_entry].0.clone();
                let mut node_remove = false;
                let mut node_move: Option<i32> = None;

                match item_info.get(item) {
                    Some((display, cost, token_cost, _, _)) => {
                        ui.label(egui::RichText::new(display).strong());
                        ui.label(format!("({})", item));
                        ui.separator();
                        // Vanilla cost for the modified indicator and reset.
                        let vanilla_c = app.vanilla_catalog.as_ref().and_then(|vc| {
                            vc.by_name.get(item).map(|&i| &vc.loot_defs[i])
                        });
                        let flag_changed = vanilla_scripts
                            .get(sel_node)
                            .and_then(|v| v.get(sel_entry))
                            .map(|(vf, _)| vf != &flag)
                            .unwrap_or(true);
                        ui.horizontal(|ui| {
                            if flag_changed {
                                ui.colored_label(CHANGED_COLOR, "Flag:");
                            } else {
                                ui.label("Flag:");
                            }
                            ui.text_edit_singleline(&mut flag).on_hover_text(
                                "Optional flag required to see this item (empty = always for sale)",
                            );
                            if flag_changed && ui.button("↺").clicked() {
                                if let Some((vf, _)) = vanilla_scripts
                                    .get(sel_node)
                                    .and_then(|v| v.get(sel_entry))
                                {
                                    flag = vf.clone();
                                }
                            }
                        });
                        let cost_changed = vanilla_c
                            .map(|v| (v.cost - *cost).abs() > 0.001)
                            .unwrap_or(true);
                        ui.horizontal(|ui| {
                            if cost_changed {
                                ui.colored_label(CHANGED_COLOR, "Cost (silver):");
                            } else {
                                ui.label("Cost (silver):");
                            }
                            let mut new_cost = *cost;
                            if ui
                                .add(egui::DragValue::new(&mut new_cost).speed(1.0))
                                .changed()
                            {
                                actions.push(ShopAction::SetCost {
                                    item: item.clone(),
                                    cost: new_cost,
                                });
                            }
                            if cost_changed {
                                if let Some(v) = vanilla_c {
                                    if ui.button("↺").clicked() {
                                        actions.push(ShopAction::SetCost {
                                            item: item.clone(),
                                            cost: v.cost,
                                        });
                                    }
                                }
                            }
                        });
                        let token_changed = vanilla_c
                            .map(|v| v.token_cost != *token_cost)
                            .unwrap_or(true);
                        ui.horizontal(|ui| {
                            if token_changed {
                                ui.colored_label(CHANGED_COLOR, "Token cost:");
                            } else {
                                ui.label("Token cost:");
                            }
                            let mut new_token = *token_cost;
                            if ui.add(egui::DragValue::new(&mut new_token)).changed() {
                                actions.push(ShopAction::SetTokenCost {
                                    item: item.clone(),
                                    token_cost: new_token,
                                });
                            }
                            if token_changed {
                                if let Some(v) = vanilla_c {
                                    if ui.button("↺").clicked() {
                                        actions.push(ShopAction::SetTokenCost {
                                            item: item.clone(),
                                            token_cost: v.token_cost,
                                        });
                                    }
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new(
                                "Cost changes apply to the item everywhere (same as the Items tab).",
                            )
                            ,
                        );
                    }
                    None => {
                        ui.label(format!("? {} (unknown item)", item));
                    }
                }
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Up").clicked() {
                        node_move = Some(-1);
                    }
                    if ui.button("Down").clicked() {
                        node_move = Some(1);
                    }
                    if ui
                        .button("Remove")
                        .on_hover_text("Remove from this merchant")
                        .clicked()
                    {
                        node_remove = true;
                    }
                });
                if flag != entries[sel_entry].0 {
                    actions.push(ShopAction::SetFlag {
                        npc: npc_idx,
                        node: sel_node,
                        entry: sel_entry,
                        flag,
                    });
                }
                if node_remove {
                    actions.push(ShopAction::RemoveEntry {
                        npc: npc_idx,
                        node: sel_node,
                        entry: sel_entry,
                    });
                    app.shop_selected_entry = None;
                }
                if let Some(d) = node_move {
                    actions.push(ShopAction::MoveEntry {
                        npc: npc_idx,
                        node: sel_node,
                        entry: sel_entry,
                        delta: d,
                    });
                    if let Some((n, e)) = app.shop_selected_entry {
                        let new_e = e as i32 + d;
                        if new_e >= 0 {
                            app.shop_selected_entry = Some((n, new_e as usize));
                        }
                    }
                }
            });

        // Central: one flat shelf per merchant (all shop nodes merged).
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ui.style()).inner_margin(2.0))
            .show_inside(ui, |ui| {
            // Flatten all shop nodes into (node_idx, entry_idx, flag, item).
            let mut flat: Vec<(usize, usize, String, String)> = Vec::new();
            for (node_idx, node) in npc_owned.nodes.iter().enumerate() {
                if !is_shop_node(node) {
                    continue;
                }
                for (e_idx, (flag, item)) in split_script(&node.store_script)
                    .iter()
                    .map(|l| parse_entry(l))
                    .enumerate()
                {
                    flat.push((node_idx, e_idx, flag, item));
                }
            }
            let mut node_add = false;
            let mut node_reset = false;

            let heading = match location_of(&app.merchant_locations, &npc_owned.name) {
                Some(loc) => format!("{} ({}) ({})", npc_owned.name, flat.len(), loc),
                None => format!("{} ({})", npc_owned.name, flat.len()),
            };
            ui.heading(heading);
            ui.horizontal(|ui| {
                if ui
                    .button("+ Add item")
                    .on_hover_text("Add an item (including custom ones) to this merchant")
                    .clicked()
                {
                    node_add = true;
                }
                if ui
                    .button("Reset to vanilla")
                    .on_hover_text("Restore the vanilla shop list for this merchant")
                    .clicked()
                {
                    node_reset = true;
                }
                let multi_count = app
                    .shop_selected_entries_multi
                    .get(&npc_idx)
                    .map_or(0, |s| s.len());
                if multi_count > 1
                    && ui
                        .button(format!("Remove selected ({})", multi_count))
                        .on_hover_text("Remove the selected entries from this merchant")
                        .clicked()
                {
                    if let Some(sel) = app.shop_selected_entries_multi.get_mut(&npc_idx) {
                        for (n, e) in sel.iter() {
                            actions.push(ShopAction::RemoveEntry {
                                npc: npc_idx,
                                node: *n,
                                entry: *e,
                            });
                        }
                        sel.clear();
                    }
                    app.shop_selected_entry = None;
                }
                ui.checkbox(&mut app.shop_show_only_modified, "Show only modified");
                crate::tabs::multisel::mouse_help_button(ui, &[]);
            });

            // "Modified" = the entry differs from the vanilla store script for that node, the item is in the sell-in-all-shops list, or the item's cost differs from vanilla.
            let is_modified = |app: &ResalinatedApp, node_idx: usize, e_idx: usize, flag: &str, item: &str| -> bool {
                if app.shop_additions.contains_key(item) {
                    return true;
                }
                if vanilla_scripts
                    .get(node_idx)
                    .and_then(|v| v.get(e_idx))
                    .map(|(vf, vi)| vf != flag || vi != item)
                    .unwrap_or(true)
                {
                    return true;
                }
                if let Some((_, cost, token_cost, _, _)) = item_info.get(item) {
                    if let Some(v) = app.vanilla_catalog.as_ref().and_then(|vc| vc.by_name.get(item).map(|&i| &vc.loot_defs[i])) {
                        if (v.cost - *cost).abs() > 0.001 || v.token_cost != *token_cost {
                            return true;
                        }
                    }
                }
                false
            };
            // Shelf: grid of item cells, like the in-game shop window.
            // Columns are pinned to the image button's real width (icon + button padding), so the wrap count matches the grid's per-cell advance and a column can never spill under the edit sidebar, even while dragging it.
            ui.style_mut().interaction.selectable_labels = false;
            // Selection gesture state (click / ctrl+click / shift+click / shift+drag box).
            let mut gsel = std::mem::take(&mut app.shop_shelf_grid_sel);
            gsel.begin(ui);
            // Full display order (respecting the show-only filter) so shift+click ranges work across scrolled-out entries.
            let show_only = app.shop_show_only_modified;
            gsel.display_order.clear();
            for (node_idx, e_idx, flag, item) in &flat {
                if show_only && !is_modified(app, *node_idx, *e_idx, flag, item) {
                    continue;
                }
                gsel.display_order.push((*node_idx, *e_idx));
            }
            egui::ScrollArea::vertical()
                .scroll_source(crate::tabs::multisel::grid_scroll_source(ui))
                .auto_shrink([false; 2])
                .show_viewport(ui, |ui, viewport| {
                    let icon = app.config.item_icon_size;
                    let btn_w = icon + 2.0 * ui.spacing().button_padding.x;
                    let cell_w = btn_w + 8.0;
                    let cols = ((ui.available_width() / cell_w).floor() as usize).max(1);
                    let show_only = app.shop_show_only_modified;
                    // Only rows whose y-range intersects the visible viewport are laid out each frame.
                    // Culled cells still advance the grid cursor via allocate_space with the exact cell size, so positions, row heights and the scrollbar stay exact while the widget count stays proportional to the viewport.
                    let label_h = ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(app.config.grid_font_size)));
                    let spacing_y = ui.spacing().item_spacing.y;
                    let pad_y = 2.0 * ui.spacing().button_padding.y;
                    let overscan = 3.0 * (icon + 2.0 * ui.spacing().button_padding.x);
                    let vp_min = viewport.min.y - overscan;
                    let vp_max = viewport.max.y + overscan;
                    egui::Grid::new(("shop_shelf", npc_idx))
                        .spacing([8.0, 8.0])
                        .min_col_width(btn_w)
                        .max_col_width(btn_w)
                        .show(ui, |ui| {
                            let mut vis = 0usize;
                            let mut row_y = 0.0f32;
                            let mut row_h = 0.0f32;
                            for (node_idx, e_idx, flag, item) in &flat {
                                if show_only && !is_modified(app, *node_idx, *e_idx, flag, item) {
                                    continue;
                                }
                                let def = app
                                    .working_catalog
                                    .as_ref()
                                    .and_then(|c| c.by_name.get(item).map(|&i| &c.loot_defs[i]));
                                let has_icon = def
                                    .and_then(|d| {
                                        app.item_atlas.as_ref().and_then(|a| a.icon_uv(d))
                                    })
                                    .is_some()
                                    || def.is_some_and(|d| {
                                        let cap = app.image_editor.capacity as i32;
                                        cap > 0
                                            && d.img >= cap
                                            && app
                                                .image_editor
                                                .icons
                                                .iter()
                                                .any(|(local, _)| *local == d.img - cap)
                                    });
                                let display = item_info
                                    .get(item)
                                    .map(|(d, _, _, _, _)| d.as_str())
                                    .unwrap_or("?");
                                let word_count = display.split_whitespace().count();
                                // Image buttons are icon_size + button frame margins tall; placeholders are icon_size.
                                let cell_h = (if has_icon { icon + pad_y } else { icon })
                                    + word_count as f32 * (label_h + spacing_y)
                                    + label_h;
                                row_h = row_h.max(cell_h);
                                let row_visible = row_y + row_h >= vp_min && row_y <= vp_max;
                                if !row_visible {
                                    ui.allocate_space(egui::vec2(btn_w, cell_h));
                                } else {
                                    ui.vertical(|ui| {
                                        let selected =
                                            app.shop_selected_entry == Some((*node_idx, *e_idx))
                                            || app
                                                .shop_selected_entries_multi
                                                .get(&npc_idx)
                                                .is_some_and(|s| {
                                                    s.contains(&(*node_idx, *e_idx))
                                                })
                                            || gsel.is_box_hit(&(*node_idx, *e_idx));
                                        let response = crate::tabs::items::draw_image_button(
                                            ui,
                                            app.item_atlas.as_ref(),
                                            def,
                                            icon,
                                            &app.image_editor,
                                        );
                                        gsel.cell(response.rect, (*node_idx, *e_idx));
                                        crate::tabs::multisel::paint_sel_outline(
                                            ui,
                                            response.rect,
                                            selected,
                                        );
                                        // Clamp the labels to the button width so long
                                        // names never widen the cell.
                                        ui.set_max_width(response.rect.width());
                                        match item_info.get(item) {
                                            Some((display, cost, token_cost, _, _)) => {
                                                crate::tabs::items::add_item_label(
                                                    ui,
                                                    display,
                                                    app.config.grid_font_size,
                                                    selected,
                                                );
                                                let cost_str = if *token_cost > 0 {
                                                    format!("{} tokens", token_cost)
                                                } else {
                                                    format!("{} silver", cost)
                                                };
                                                ui.label(
                                                    egui::RichText::new(cost_str)
                                                        ,
                                                );
                                            }
                                            None => {
                                                ui.label(
                                                    egui::RichText::new("?")
                                                        ,
                                                );
                                            }
                                        }
                                    });
                                }
                                vis += 1;
                                if vis % cols == 0 {
                                    ui.end_row();
                                    row_y += row_h + spacing_y;
                                    row_h = 0.0;
                                }
                            }
                        });

                    gsel.update_target();
                    gsel.paint(ui);
                    // The shelf keeps one multi-set per merchant, so apply the gesture into the per-npc set.
                    let mut single = app.shop_selected_entry;
                    let multi = app.shop_selected_entries_multi.entry(npc_idx).or_default();
                    gsel.end(ui, multi, &mut single);
                    app.shop_selected_entry = single;
                    app.shop_shelf_grid_sel = gsel;
                });
            let _ = flat;
            ui.add_space(8.0);

            if node_add {
                // Add to the first shop node of this merchant.
                if let Some((n, _, _, _)) = flat.first() {
                    picker_target = Some((npc_idx, *n));
                }
            }
            if node_reset {
                for (node_idx, _, _, _) in &flat {
                    actions.push(ShopAction::ResetNode {
                        npc: npc_idx,
                        node: *node_idx,
                    });
                }
            }
        });
        let _ = edit_panel;
    });

    apply_actions(app, actions);

    // Open the picker for a merchant node if requested.
    if let Some(target) = picker_target {
        app.shop_picker_target = Some(target);
        app.shop_picker_open = true;
        app.shop_picker_search.clear();
        app.shop_picker_focus = true;
    }

    // "Add item" picker: same item grid as the Items tab (grouped by type-subtype category, both scrollbars) with a search that auto-focuses on open.
    if app.shop_picker_open {
        let mut open = app.shop_picker_open;
        let mut chosen: Option<String> = None;
        egui::Window::new("Add item to shop")
            .collapsible(false)
            .resizable(true)
            .default_width(620.0)
            .default_height(480.0)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .open(&mut open)
            .show(ui.ctx(), |ui| {
                // Search, auto-focused when the window opens.
                ui.horizontal(|ui| {
                    ui.label("Search:");
                    let resp = ui.text_edit_singleline(&mut app.shop_picker_search);
                    if app.shop_picker_focus {
                        resp.request_focus();
                        app.shop_picker_focus = false;
                    }
                });
                ui.separator();

                // Entries already present in the target node (to mark them).
                let existing: Vec<String> = app
                    .shop_picker_target
                    .and_then(|(npc, node)| {
                        app.working_dialog
                            .as_ref()
                            .and_then(|d| d.npcs.get(npc))
                            .and_then(|n| n.nodes.get(node))
                            .map(|nd| split_script(&nd.store_script))
                    })
                    .unwrap_or_default();

                let needle = app.shop_picker_search.to_lowercase();
                let filtered: Vec<&(String, String, f32, i32, i32, i32)> = all_items
                    .iter()
                    .filter(|(name, display, _, _, _, _)| {
                        needle.is_empty()
                            || name.to_lowercase().contains(&needle)
                            || display.to_lowercase().contains(&needle)
                    })
                    .collect();

                // Group by type-subtype category, exactly like the Items tab.
                let mut grouped: std::collections::BTreeMap<
                    String,
                    Vec<&(String, String, f32, i32, i32, i32)>,
                > = std::collections::BTreeMap::new();
                for item in filtered {
                    let cat = format!(
                        "{} - {}",
                        sas2_parser::loot_names::get_type_name(item.4),
                        sas2_parser::loot_names::get_subtype_name(item.4, item.5)
                    );
                    grouped.entry(cat).or_default().push(item);
                }

                egui::ScrollArea::both()
                    .auto_shrink([false; 2])
                    .show_viewport(ui, |ui, viewport| {
                        // Only items whose x-range intersects the visible viewport are laid out each frame.
                        // Culled items still advance the grid cursor via allocate_space with the exact cell size, so positions, row heights and the scrollbar stay exact while the widget count stays proportional to the viewport.
                        let icon_size = app.config.item_icon_size;
                        let label_h = ui.fonts_mut(|f| {
                            f.row_height(&egui::FontId::proportional(app.config.grid_font_size))
                        });
                        let spacing_y = ui.spacing().item_spacing.y;
                        let pad_x = 2.0 * ui.spacing().button_padding.x;
                        let pad_y = 2.0 * ui.spacing().button_padding.y;
                        let overscan = 3.0 * (icon_size + pad_x);
                        let vp_min = viewport.min.x - overscan;
                        let vp_max = viewport.max.x + overscan;

                        for (cat, entries) in grouped {
                            ui.style_mut().interaction.selectable_labels = false;
                            ui.label(
                                egui::RichText::new(&cat)
                                    .strong()
                                    .size(app.config.category_font_size),
                            );
                            egui::Grid::new(("shop_pick_grid", cat))
                                .spacing([8.0, 8.0])
                                .show(ui, |ui| {
                                    let mut x = 0.0f32;
                                    for (name, display, cost, token_cost, _, _) in entries {
                                        let def = app.working_catalog.as_ref().and_then(|cat| {
                                            cat.by_name
                                                .get(name)
                                                .and_then(|&i| cat.loot_defs.get(i))
                                        });
                                        let has_icon = def
                                            .and_then(|d| {
                                                app.item_atlas.as_ref().and_then(|a| a.icon_uv(d))
                                            })
                                            .is_some()
                                            || def.is_some_and(|d| {
                                                let cap = app.image_editor.capacity as i32;
                                                cap > 0
                                                    && d.img >= cap
                                                    && app
                                                        .image_editor
                                                        .icons
                                                        .iter()
                                                        .any(|(local, _)| *local == d.img - cap)
                                            });
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
                                            let already = existing.contains(name);
                                            let response = draw_item_cell(
                                                ui,
                                                app,
                                                name,
                                                display,
                                                *cost,
                                                *token_cost,
                                                icon_size,
                                                false,
                                            );
                                            if already {
                                                ui.label(
                                                    egui::RichText::new("in shop")
                                                        .color(egui::Color32::LIGHT_GREEN),
                                                );
                                            }
                                            if response.clicked() {
                                                chosen = Some(name.clone());
                                            }
                                        });
                                    }
                                });
                            ui.add_space(8.0);
                        }
                    });
            });
        app.shop_picker_open = open;
        if let Some(name) = chosen {
            if let Some((npc, node)) = app.shop_picker_target {
                apply_actions(
                    app,
                    vec![ShopAction::AddItem {
                        npc,
                        node,
                        item: name,
                    }],
                );
            } else {
                app.shop_additions.entry(name).or_default();
            }
            app.shop_picker_open = false;
            app.shop_picker_target = None;
        }
    }
}
