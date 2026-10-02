use crate::app::ResalinatedApp;
use eframe::egui;
use egui::Ui;
use sas2_parser::loot_catalog::{LootCatalog, LootDef};
use std::collections::HashSet;

/// One entry of a player item list: catalog icon, display name and pierce key.
struct ItemEntry {
    key: String,
    display: String,
    img: i32,
}

/// One enemy: display name, sprite source, pierce key and role group.
struct MonsterEntry {
    key: String,
    display: String,
    char_def: String,
    texture: String,
    group: String,
    group_order: usize,
}

struct PlayerItems {
    magic: Vec<ItemEntry>,
    ranged: Vec<ItemEntry>,
    thrown: Vec<ItemEntry>,
}

fn collect(
    catalog: &LootCatalog,
    search: &str,
    filter: impl Fn(&LootDef) -> bool,
) -> Vec<ItemEntry> {
    catalog
        .loot_defs
        .iter()
        .filter(|d| filter(d))
        .filter(|d| {
            search.is_empty()
                || d.name.to_lowercase().contains(search)
                || d.title.iter().any(|t| t.to_lowercase().contains(search))
        })
        .map(|d| ItemEntry {
            key: format!("item:{}", d.name),
            display: d
                .title
                .first()
                .filter(|t| !t.is_empty())
                .cloned()
                .unwrap_or_else(|| d.name.clone()),
            img: d.img,
        })
        .collect()
}

fn collect_player_items(catalog: &LootCatalog, search: &str) -> PlayerItems {
    PlayerItems {
        // Every LootMagic rune.
        magic: collect(catalog, search, |d| d.type_ == 7),
        // LootRanged: bows, crossbows, throwing weapons and channeling rods.
        ranged: collect(catalog, search, |d| d.type_ == 2),
        // Consumables with the "+Throwable" flag: bombs and decoctions.
        thrown: collect(catalog, search, |d| d.type_ == 3 && d.flags.contains(&3)),
    }
}

/// Item tile (icon, name, pierce state).
fn item_tile(
    ui: &mut Ui,
    app: &ResalinatedApp,
    entry: &ItemEntry,
    icon_size: f32,
    enabled: bool,
) -> egui::Response {
    let capacity = app.image_editor.capacity as i32;
    let custom = if capacity > 0 && entry.img >= capacity {
        app.image_editor
            .icons
            .iter()
            .find(|(i, _)| *i == entry.img - capacity)
            .map(|(_, h)| h.clone())
    } else {
        None
    };

    let response = if let Some(handle) = custom {
        ui.add(egui::Button::image(
            egui::Image::from_texture(&handle).fit_to_exact_size(egui::vec2(icon_size, icon_size)),
        ))
    } else if let Some(uv) = app.item_atlas.as_ref().and_then(|a| a.icon_uv_for_img(entry.img)) {
        let atlas = app.item_atlas.as_ref().unwrap();
        ui.add(egui::Button::image(
            egui::Image::from_texture(&atlas.texture)
                .fit_to_exact_size(egui::vec2(icon_size, icon_size))
                .uv(uv),
        ))
    } else {
        ui.allocate_response(egui::vec2(icon_size, icon_size), egui::Sense::click())
    };

    crate::tabs::monster_grid::monster_label(ui, &entry.display, tile_font(ui), enabled);
    pierce_state(ui, enabled);
    response
}

fn tile_font(ui: &Ui) -> f32 {
    ui.style().text_styles[&egui::TextStyle::Small].size
}

fn pierce_state(ui: &mut Ui, enabled: bool) {
    let small = tile_font(ui);
    ui.label(
        egui::RichText::new(if enabled { "pierce on" } else { "off" })
            .size(small - 1.0)
            .color(if enabled {
                egui::Color32::LIGHT_GREEN
            } else {
                egui::Color32::GRAY
            }),
    );
}

/// Buttons of one side, placed next to its search box.
#[derive(Default)]
struct SideActions {
    enable_selected: bool,
    disable_selected: bool,
    enable_all: bool,
    disable_all: bool,
}

/// Search box + Enable/Disable (selection) and Enable All/Disable All (every entry of the side,
/// filter and selection ignored).
fn side_controls(
    ui: &mut Ui,
    search: &mut String,
    hint: &str,
    total: usize,
    selected: usize,
    toggle_on_click: &mut bool,
    config_save_timer: &mut f32,
) -> SideActions {
    let mut actions = SideActions::default();
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.add(
            egui::TextEdit::singleline(search)
                .hint_text(hint)
                .desired_width(180.0),
        );
        let has_selection = selected > 0;
        if ui
            .add_enabled(has_selection, egui::Button::new("Enable"))
            .on_hover_text("Turn pierce on for the selected entries")
            .clicked()
        {
            actions.enable_selected = true;
        }
        if ui
            .add_enabled(has_selection, egui::Button::new("Disable"))
            .on_hover_text("Turn pierce off for the selected entries")
            .clicked()
        {
            actions.disable_selected = true;
        }
        ui.label(
            egui::RichText::new(format!("{} selected", selected)).color(egui::Color32::GRAY),
        );
        if ui
            .button("Enable All")
            .on_hover_text(format!(
                "Turn pierce on for all {} entries of this side, ignoring the filter and the selection",
                total
            ))
            .clicked()
        {
            actions.enable_all = true;
        }
        if ui
            .button("Disable All")
            .on_hover_text(format!(
                "Turn pierce off for all {} entries of this side, ignoring the filter and the selection",
                total
            ))
            .clicked()
        {
            actions.disable_all = true;
        }
        if ui
            .checkbox(toggle_on_click, "Toggle on selection")
            .on_hover_text(
                "On: clicking toggles pierce for that entry. \nOff: clicking only selects it; use Enable / Disable.",
            )
            .changed()
        {
            *config_save_timer = 0.1;
        }
        // Same shared help text as every other grid; no tab-specific lines.
        crate::tabs::multisel::mouse_help_button(ui, &[]);
    });
    actions
}

/// Click behaviour of a pierce tile: enable-only when "Enable on click" is on, otherwise toggle.
/// Click behaviour of a pierce tile: toggle when "Toggle on selection" is on, otherwise the click
/// only selects the entry and the Enable All / Disable All buttons change the state.
fn set_pierce(
    set: &mut HashSet<String>,
    key: &str,
    currently_enabled: bool,
    toggle_on_click: bool,
) {
    if !toggle_on_click {
        return;
    }
    if currently_enabled {
        set.remove(key);
    } else {
        set.insert(key.to_string());
    }
}

pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    ui.label(
        "Magic and ranged projectiles normally disappear on the first enemy they touch. With \
         pierce on, they fly through enemies, hit each of them once and are only stopped by \
         terrain. Player projectiles are chosen per item, enemy projectiles per monster.",
    );
    ui.add_space(6.0);

    let search = app.pierce_item_search.to_lowercase();
    let enemy_search = app.pierce_enemy_search.to_lowercase();
    let player_items = app
        .working_catalog
        .as_ref()
        .map(|c| collect_player_items(c, &search))
        .unwrap_or(PlayerItems {
            magic: Vec::new(),
            ranged: Vec::new(),
            thrown: Vec::new(),
        });
    let monsters: Vec<MonsterEntry> = app
        .working_monster_catalog
        .as_ref()
        .map(|cat| {
            cat.monsters
                .iter()
                .filter(|m| m.type_ == 1)
                .filter(|m| {
                    enemy_search.is_empty()
                        || m.name.to_lowercase().contains(&enemy_search)
                        || m.titles
                            .iter()
                            .any(|t| t.to_lowercase().contains(&enemy_search))
                })
                .map(|m| {
                    let (group, group_order) =
                        crate::tabs::hazeburnt_spawns::role_group(m);
                    MonsterEntry {
                        key: format!("monster:{}", m.name),
                        display: m
                            .titles
                            .first()
                            .filter(|t| !t.is_empty())
                            .cloned()
                            .unwrap_or_else(|| m.name.clone()),
                        char_def: m.def.clone(),
                        texture: m.texture.clone(),
                        group,
                        group_order,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let icon_size = ui.style().text_styles[&egui::TextStyle::Body].size * 3.4;

    egui::ScrollArea::both()
        .scroll_source(crate::tabs::multisel::grid_scroll_source(ui))
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            // ---------------- Player ----------------
            ui.heading("Player Projectiles");
            let player_keys: Vec<String> = player_items
                .magic
                .iter()
                .chain(&player_items.ranged)
                .chain(&player_items.thrown)
                .map(|e| e.key.clone())
                .collect();
            let player_selected: Vec<String> = app
                .projectile_selected_multi
                .iter()
                .chain(app.projectile_selected_single.iter())
                .filter(|i| **i < player_keys.len())
                .filter_map(|i| player_keys.get(*i).cloned())
                .collect();
            let player_actions = side_controls(
                ui,
                &mut app.pierce_item_search,
                "item name",
                player_keys.len(),
                player_selected.len(),
                &mut app.config.projectile_toggle_on_click,
                &mut app.config_save_timer,
            );

            if player_actions.enable_selected || player_actions.disable_selected {
                for key in &player_selected {
                    if player_actions.enable_selected {
                        app.pierce_player.insert(key.clone());
                    }
                    if player_actions.disable_selected {
                        app.pierce_player.remove(key);
                    }
                }
            }
            if player_actions.enable_all {
                for key in &player_keys {
                    app.pierce_player.insert(key.clone());
                }
            }
            if player_actions.disable_all {
                for key in &player_keys {
                    app.pierce_player.remove(key);
                }
            }

            let mut gsel = std::mem::take(&mut app.projectile_grid_sel);
            gsel.begin(ui);
            gsel.display_order.clear();
            // Cell keys are indices into the concatenation of every grid, in draw order.
            let all_keys: Vec<String> = player_items
                .magic
                .iter()
                .chain(&player_items.ranged)
                .chain(&player_items.thrown)
                .map(|e| e.key.clone())
                .chain(monsters.iter().map(|m| m.key.clone()))
                .collect();
            gsel.display_order.extend(0..all_keys.len());

            for (title, hint, list) in [
                (
                    "Magic (runes)",
                    "Every rune: the projectiles of the rune being cast pierce.",
                    &player_items.magic,
                ),
                (
                    "Ranged weapons",
                    "Bows, crossbows and throwing weapons: the shots of the equipped weapon pierce.",
                    &player_items.ranged,
                ),
                (
                    "Thrown items",
                    "Throwable consumables: the projectiles of the item you throw pierce.",
                    &player_items.thrown,
                ),
            ] {
                let keys: Vec<String> = list.iter().map(|e| e.key.clone()).collect();
                let base = match title {
                    t if t.starts_with("Magic") => 0,
                    t if t.starts_with("Ranged") => player_items.magic.len(),
                    _ => player_items.magic.len() + player_items.ranged.len(),
                };
                ui.add_space(6.0);
                ui.label(
                    egui::RichText::new(format!("{} ({})", title, keys.len()))
                        .strong()
                        .size(ui.style().text_styles[&egui::TextStyle::Body].size),
                )
                .on_hover_text(hint);
                ui.horizontal_wrapped(|ui| {
                    ui.style_mut().interaction.selectable_labels = false;
                    for (i, entry) in list.iter().enumerate() {
                        let idx = base + i;
                        let enabled = app.pierce_player.contains(&entry.key);
                        let response = ui
                            .vertical(|ui| item_tile(ui, app, entry, icon_size, enabled))
                            .inner;
                        gsel.cell(response.rect, idx);
                        // Green border: pierce is on for this entry.
                        crate::tabs::multisel::paint_sel_outline(ui, response.rect, enabled);
                        let ui_selected = app.projectile_selected_multi.contains(&idx)
                            || app.projectile_selected_single == Some(idx)
                            || gsel.is_box_hit(&idx);
                        crate::tabs::multisel::paint_outline_colored(
                            ui,
                            response.rect,
                            ui_selected.then_some(egui::Color32::from_rgb(230, 150, 60)),
                        );
                        let hover = format!(
                            "{}\n{}\nClick to toggle pierce, right-click or drag to select",
                            entry.display, entry.key
                        );
                        if response.on_hover_text(hover).clicked() {
                            set_pierce(
                                &mut app.pierce_player,
                                &entry.key,
                                enabled,
                                app.config.projectile_toggle_on_click,
                            );
                        }
                    }
                });
                if list.is_empty() {
                    ui.label(
                        egui::RichText::new("no entry matches the search")
                            .size(tile_font(ui))
                            .color(egui::Color32::GRAY),
                    );
                }
                ui.add_space(6.0);
            }

            ui.separator();

            // ---------------- Enemy ----------------
            ui.heading("Enemy Projectiles");
            let player_count = player_keys.len();
            let monster_keys: Vec<String> = monsters.iter().map(|m| m.key.clone()).collect();
            let monster_selected: Vec<String> = app
                .projectile_selected_multi
                .iter()
                .chain(app.projectile_selected_single.iter())
                .filter(|i| **i >= player_count)
                .filter_map(|i| monster_keys.get(*i - player_count).cloned())
                .collect();
            let monster_actions = side_controls(
                ui,
                &mut app.pierce_enemy_search,
                "monster name",
                monster_keys.len(),
                monster_selected.len(),
                &mut app.config.projectile_toggle_on_click,
                &mut app.config_save_timer,
            );
            ui.label(
                egui::RichText::new(format!(
                    "{} of {} shown monsters piercing",
                    monsters
                        .iter()
                        .filter(|m| app.pierce_enemy.contains(&m.key))
                        .count(),
                    monsters.len()
                ))
                .color(egui::Color32::GRAY),
            );
            if monster_actions.enable_selected || monster_actions.disable_selected {
                for key in &monster_selected {
                    if monster_actions.enable_selected {
                        app.pierce_enemy.insert(key.clone());
                    }
                    if monster_actions.disable_selected {
                        app.pierce_enemy.remove(key);
                    }
                }
            }
            if monster_actions.enable_all {
                for key in &monster_keys {
                    app.pierce_enemy.insert(key.clone());
                }
            }
            if monster_actions.disable_all {
                for key in &monster_keys {
                    app.pierce_enemy.remove(key);
                }
            }

            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!("Monsters ({})", monster_keys.len()))
                    .strong()
                    .size(ui.style().text_styles[&egui::TextStyle::Body].size),
            )
            .on_hover_text("Every regular enemy: the projectiles it fires pierce.");
            // Role groups, ordered like the Hazeburnt Spawns "Role" grouping.
            let mut monster_groups: std::collections::BTreeMap<(usize, String), Vec<usize>> =
                std::collections::BTreeMap::new();
            for (mi, entry) in monsters.iter().enumerate() {
                monster_groups
                    .entry((entry.group_order, entry.group.clone()))
                    .or_default()
                    .push(mi);
            }
            let monster_base = all_keys.len() - monsters.len();
            for ((_, group), indices) in &monster_groups {
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(format!("{} ({})", group, indices.len()))
                        .strong()
                        .size(ui.style().text_styles[&egui::TextStyle::Body].size),
                );
                ui.horizontal_wrapped(|ui| {
                ui.style_mut().interaction.selectable_labels = false;
                for &mi in indices {
                    let Some(entry) = monsters.get(mi) else { continue };
                    let idx = monster_base + mi;
                    let enabled = app.pierce_enemy.contains(&entry.key);
                    let tex = if entry.texture.is_empty() {
                        None
                    } else {
                        app.monster_texture_cache
                            .get_or_assemble(ui.ctx(), &entry.char_def, &entry.texture)
                    };
                    let response = ui
                        .vertical(|ui| {
                            let response = if let Some(tex) = &tex {
                                ui.add(egui::Button::image(
                                    egui::Image::from_texture(&tex.clone())
                                        .fit_to_exact_size(egui::vec2(icon_size, icon_size)),
                                ))
                            } else {
                                ui.allocate_response(
                                    egui::vec2(icon_size, icon_size),
                                    egui::Sense::click(),
                                )
                            };
                            crate::tabs::monster_grid::monster_label(
                                ui,
                                &entry.display,
                                tile_font(ui),
                                enabled,
                            );
                            pierce_state(ui, enabled);
                            response
                        })
                        .inner;
                    gsel.cell(response.rect, idx);
                    // Green border: pierce is on for this monster.
                    crate::tabs::multisel::paint_sel_outline(ui, response.rect, enabled);
                    let ui_selected = app.projectile_selected_multi.contains(&idx)
                        || app.projectile_selected_single == Some(idx)
                        || gsel.is_box_hit(&idx);
                    crate::tabs::multisel::paint_outline_colored(
                        ui,
                        response.rect,
                        ui_selected.then_some(egui::Color32::from_rgb(230, 150, 60)),
                    );
                    let hover = format!(
                        "{}\n{}\nClick to toggle pierce, right-click or drag to select",
                        entry.display, entry.key
                    );
                    if response.on_hover_text(hover).clicked() {
                        set_pierce(
                            &mut app.pierce_enemy,
                            &entry.key,
                            enabled,
                            app.config.projectile_toggle_on_click,
                        );
                    }
                }
                });
            }

            gsel.update_target();
            gsel.paint(ui);
            gsel.end(
                ui,
                &mut app.projectile_selected_multi,
                &mut app.projectile_selected_single,
            );
            app.projectile_grid_sel = gsel;

        });
}
