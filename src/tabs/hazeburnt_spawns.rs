use crate::app::ResalinatedApp;
use crate::tabs::monster_grid::{self, GridMetrics};
use eframe::egui;
use egui::Ui;
use sas2_parser::monster_catalog::{MonsterCatalog, MonsterFieldValue};
use std::collections::{HashMap, HashSet};

/// How the monster grid is grouped.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GroupBy {
    Element,
    Role,
    Family,
    VanillaArea,
    Size,
    TypeSubType,
}

impl GroupBy {
    pub const ALL: [GroupBy; 6] = [
        GroupBy::Element,
        GroupBy::Role,
        GroupBy::Family,
        GroupBy::VanillaArea,
        GroupBy::Size,
        GroupBy::TypeSubType,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GroupBy::Element => "Element",
            GroupBy::Role => "Role",
            GroupBy::Family => "Name family",
            GroupBy::VanillaArea => "Vanilla area",
            GroupBy::Size => "Size",
            GroupBy::TypeSubType => "Type - SubType",
        }
    }

    pub fn hover(self) -> &'static str {
        match self {
            GroupBy::Element => {
                "Group by the game's Elem| flags (Fire, Ice, Undead, Demon, ...), so enemies of \
                 the same element sit together."
            }
            GroupBy::Role => {
                "Group by combat role from the game's flags: Boss, Mage, Minion, Flyer, Giant, \
                 Warping, Leaping, Undead, ..."
            }
            GroupBy::Family => {
                "Group by creature family, the part of the name before the first underscore \
                 (zombie_*, burnt_*, elec_*, ...)."
            }
            GroupBy::VanillaArea => {
                "Group by the area the monster belongs to in the vanilla hazeburnt pools."
            }
            GroupBy::Size => "Group by body size (small, medium, large, giant).",
            GroupBy::TypeSubType => "Group by the monster type and sub type, like the Monsters tab.",
        }
    }
}

/// The game's element flags (MonsterMonster.GetFlagName): flag index and display name.
const ELEMENT_FLAGS: &[(i32, &str)] = &[
    (5, "Fire"),
    (6, "Water"),
    (7, "Lightning"),
    (8, "Poison"),
    (9, "Earth"),
    (10, "Time"),
    (11, "Flesh"),
    (12, "Fungus"),
    (13, "Mech"),
    (14, "Demon"),
    (15, "Dragon"),
    (16, "Books"),
    (17, "Air"),
    (18, "Ice"),
    (19, "Force"),
    (20, "Divine"),
    (21, "Light"),
    (22, "Blood"),
    (23, "Undead"),
    (24, "Mind"),
    (25, "Dark"),
    (32, "Red Lightning"),
    (33, "Red Fire"),
    (35, "Mummy"),
    (37, "Hag"),
    (42, "Ghost"),
    (44, "Blue Dragon"),
    (46, "Sky"),
    (48, "Messiah"),
    (50, "Owl"),
    (51, "Sky Lightning"),
    (54, "Heart"),
];

/// Role group of a monster (label, sort order), used by the Hazeburnt Spawns "Role" grouping and
/// by the Projectiles tab's monster list.
pub fn role_group(def: &sas2_parser::monster_catalog::MonsterDef) -> (String, usize) {
    for (order, (flag, name)) in ROLE_FLAGS.iter().enumerate() {
        if def.flags.contains(flag) {
            return (name.to_string(), order);
        }
    }
    ("Other".to_string(), ROLE_FLAGS.len())
}

/// Combat role flags in priority order (the first match wins).
const ROLE_FLAGS: &[(i32, &str)] = &[
    (34, "Boss"),
    (45, "Symbiotic Boss"),
    (30, "Hazeburnt"),
    (0, "Mage"),
    (26, "Elite Minion"),
    (27, "Special Minion"),
    (2, "Minion"),
    (3, "Flyer"),
    (4, "Giant"),
    (29, "Warping"),
    (28, "Leaping"),
    (36, "Undead"),
    (43, "Mimic"),
    (47, "Devourable"),
    (1, "Regular"),
];

/// Group key (label, sort order) of a monster for the selected grouping.
fn group_of(
    by: GroupBy,
    def: &sas2_parser::monster_catalog::MonsterDef,
    loc_strings: &[String],
) -> (String, usize) {
    match by {
        GroupBy::Element => {
            for (order, (flag, name)) in ELEMENT_FLAGS.iter().enumerate() {
                if def.flags.contains(flag) {
                    return (name.to_string(), order);
                }
            }
            // Hazeburnt monsters carry no element flag; group them by their theme instead.
            if def.flags.contains(&FLAG_HAZEBURNT) {
                return ("Hazeburnt".to_string(), ELEMENT_FLAGS.len());
            }
            ("No element".to_string(), ELEMENT_FLAGS.len() + 1)
        }
        GroupBy::Role => {
            for (order, (flag, name)) in ROLE_FLAGS.iter().enumerate() {
                if def.flags.contains(flag) {
                    return (name.to_string(), order);
                }
            }
            ("Other".to_string(), ROLE_FLAGS.len())
        }
        GroupBy::Family => {
            let family = match def.name.split_once('_') {
                Some((head, _)) => head,
                None => def.name.as_str(),
            };
            let mut chars = family.chars();
            let label = match chars.next() {
                Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str()),
                None => family.to_string(),
            };
            (label, 0)
        }
        GroupBy::VanillaArea => {
            let area = vanilla_area(def);
            if area > 0 && area <= AREA_COUNT {
                (area_label(area, loc_strings), area as usize)
            } else {
                ("Not in the vanilla pools".to_string(), AREA_COUNT as usize + 1)
            }
        }        GroupBy::Size => {
            let h = def.box_height;
            let (label, order) = if h < 100 {
                ("Small", 0)
            } else if h < 180 {
                ("Medium", 1)
            } else if h < 320 {
                ("Large", 2)
            } else {
                ("Giant", 3)
            };
            (label.to_string(), order)
        }
        GroupBy::TypeSubType => (
            format!(
                "{} - SubType {}",
                sas2_parser::monster_names::get_monster_type_name(def.type_),
                def.sub_type
            ),
            0,
        ),
    }
}

/// Areas the hazeburnt manager tracks (areaHazeburnts holds 9 lists, index 0 is unused).
pub const AREA_COUNT: i32 = 8;

/// Loc string indices of the area names (MissionAreas.GetAreaName).
fn area_loc_index(area: i32) -> Option<usize> {
    let idx = match area {
        0 => 361,
        1 => 400,
        2 => 401,
        3 => 381,
        4 => 370,
        5 => 364,
        6 => 378,
        7 | 8 => 352,
        _ => return None,
    };
    Some(idx as usize)
}

/// Strips the game's color markup ("[G]Ashbourne Village[g]") from a loc string.
fn strip_color_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '[' => in_tag = true,
            ']' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// "3: Corvius' Mire" style area label, falling back to the map name when strings.ztx is unavailable.
pub fn area_label(area: i32, loc_strings: &[String]) -> String {
    area_loc_index(area)
        .and_then(|i| loc_strings.get(i))
        .filter(|s| !s.is_empty())
        .map(|s| strip_color_tags(s))
        .unwrap_or_else(|| match area {
            0 => "Camp".to_string(),
            1 => "Village".to_string(),
            2 => "Wastes".to_string(),
            3 => "Swamp".to_string(),
            4 => "Mountain".to_string(),
            5 => "Copse".to_string(),
            6 => "Nowhere".to_string(),
            7 => "Arena".to_string(),
            8 => "Arena Fight".to_string(),
            _ => format!("Area {}", area),
        })
}

/// True when the monster is one of the enemies the vanilla hazeburnt system can spawn: hazeburnt
/// flagged AND assigned to one of the areas it watches. Named story monsters such as Inquisitor
/// Selet (blue_nomad) carry the hazeburnt flag too but have no area, so they are not spawnable by
/// the system and only show up with "Show all monsters".
pub fn is_vanilla_hazeburnt_pick(def: &sas2_parser::monster_catalog::MonsterDef) -> bool {
    is_hazeburnt(def) && vanilla_area(def) > 0 && vanilla_area(def) <= AREA_COUNT
}

/// Monster flag that marks a def as hazeburnt (GameMonster.hazeBurnt).
const FLAG_HAZEBURNT: i32 = 30;

/// Monster field holding the vanilla hazeburnt area (HazeburntMgr.PopulateHazeburntMonsters).
const FIELD_AREA: i32 = 63;

/// True when the monster is one of the hazeburnt variants the vanilla system spawns.
pub fn is_hazeburnt(def: &sas2_parser::monster_catalog::MonsterDef) -> bool {
    def.type_ == 1 && def.flags.contains(&FLAG_HAZEBURNT)
}

/// The vanilla hazeburnt area of a monster, or -1 when it is not part of the vanilla pools.
pub fn vanilla_area(def: &sas2_parser::monster_catalog::MonsterDef) -> i32 {
    if !is_hazeburnt(def) {
        return -1;
    }
    def.fields
        .iter()
        .find(|f| f.id == FIELD_AREA)
        .and_then(|f| match f.value {
            MonsterFieldValue::Int(v) => Some(v),
            _ => None,
        })
        .unwrap_or(-1)
}

/// Vanilla area -> monster names, the starting point shown by the tab.
pub fn vanilla_areas(cat: &MonsterCatalog) -> HashMap<i32, Vec<String>> {
    let mut out: HashMap<i32, Vec<String>> = HashMap::new();
    for def in &cat.monsters {
        let area = vanilla_area(def);
        if area > 0 && area <= AREA_COUNT {
            out.entry(area).or_default().push(def.name.clone());
        }
    }
    out
}

/// Display name of a monster (first non-empty title, falling back to the def name).
pub fn display_name(def: &sas2_parser::monster_catalog::MonsterDef) -> String {
    def.titles
        .iter()
        .find(|t| !t.is_empty())
        .cloned()
        .unwrap_or_else(|| def.name.clone())
}

/// One row of the grid: everything needed to draw a tile and toggle its pool membership.
struct Entry {
    name: String,
    display: String,
    texture: String,
    char_def: String,
    hazeburnt: bool,
    vanilla_area: i32,
    disabled: bool,
    group_label: String,
    group_order: usize,
}

/// Short note under a tile: only shown when there is something worth flagging.
/// "vanilla" marks a hazeburnt monster that belongs to the area currently open.
fn entry_note(entry: &Entry, area: i32) -> Option<(String, egui::Color32)> {
    if entry.disabled {
        Some((
            "disabled".to_string(),
            egui::Color32::from_rgb(220, 120, 120),
        ))
    } else if entry.hazeburnt && entry.vanilla_area == area {
        Some(("vanilla".to_string(), egui::Color32::GRAY))
    } else {
        None
    }
}

pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    if app.working_monster_catalog.is_none() {
        ui.label("No monster catalog loaded.");
        return;
    }

    if !app.hazeburnt_spawns_initialized {
        app.init_hazeburnt_spawns();
    }

    ui.label(
        "Choose which enemies the hazeburnt system can spawn in each area. Pools start from the \
         vanilla assignment; monsters that are not normally hazeburnt get the hazeburnt behavior \
         applied (haze pile on death, hazeburnt hostility) when they spawn from these lists.",
    );
    ui.add_space(6.0);

    // Area selector.
    ui.horizontal_wrapped(|ui| {
        ui.label("Area:");
        for area in 1..=AREA_COUNT {
            let count = app
                .hazeburnt_spawns
                .get(&area)
                .map(|names| names.len())
                .unwrap_or(0);
            let selected = app.hazeburnt_selected_area == area;
            if ui
                .selectable_label(selected, format!("{} ({})", area_label(area, &app.loc_strings), count))
                .clicked()
            {
                app.hazeburnt_selected_area = area;
            }
        }
    });

    // Filter row.
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.add(
            egui::TextEdit::singleline(&mut app.hazeburnt_search)
                .hint_text("monster name")
                .desired_width(220.0),
        );
        if ui
            .checkbox(&mut app.config.hazeburnt_show_all_monsters, "Show all monsters")
            .on_hover_text(
                "Off: only the hazeburnt monster variants are listed.\nOn: every regular enemy is listed too.",
            )
            .changed()
        {
            app.config_save_timer = 0.1;
        }
        if ui
            .button("Reset area to vanilla")
            .on_hover_text("Restore this area's vanilla hazeburnt pool")
            .clicked()
        {
            let area = app.hazeburnt_selected_area;
            if let Some(cat) = &app.working_monster_catalog {
                let vanilla = vanilla_areas(cat);
                let names: HashSet<String> =
                    vanilla.get(&area).cloned().unwrap_or_default().into_iter().collect();
                app.hazeburnt_spawns.insert(area, names);
            }
        }
        if ui
            .button("Clear area")
            .on_hover_text("No hazeburnt monsters spawn in this area")
            .clicked()
        {
            let area = app.hazeburnt_selected_area;
            app.hazeburnt_spawns.insert(area, HashSet::new());
        }
        ui.separator();
        ui.label("Group by:");
        egui::ComboBox::from_id_salt("hazeburnt_group_by")
            .selected_text(app.hazeburnt_group_by.label())
            .show_ui(ui, |ui| {
                for option in GroupBy::ALL {
                    ui.selectable_value(&mut app.hazeburnt_group_by, option, option.label())
                        .on_hover_text(option.hover());
                }
            });
        if ui
            .checkbox(&mut app.config.hazeburnt_toggle_on_click, "Toggle on selection")
            .on_hover_text(
                "On: clicking a monster toggles it in and out of this area's pool. \nOff: clicking only selects it; use Enable/Disable to change the pool.",
            )
            .changed()
        {
            app.config_save_timer = 0.1;
        }
        // Same shared help text as every other grid; no tab-specific lines.
        crate::tabs::multisel::mouse_help_button(ui, &[]);
    });

    let area = app.hazeburnt_selected_area;
    let search = app.hazeburnt_search.to_lowercase();
    let show_all = app.config.hazeburnt_show_all_monsters;
    let group_by = app.hazeburnt_group_by;

    // Build the filtered list, grouped by the selected trait.
    let entries: Vec<Entry> = app
        .working_monster_catalog
        .as_ref()
        .map(|cat| {
            cat.monsters
                .iter()
                .filter(|d| d.type_ == 1)
                .filter(|d| show_all || is_vanilla_hazeburnt_pick(d))
                .filter(|d| {
                    search.is_empty()
                        || d.name.to_lowercase().contains(&search)
                        || display_name(d).to_lowercase().contains(&search)
                })
                .map(|d| {
                    let (group_label, group_order) = group_of(group_by, d, &app.loc_strings);
                    Entry {
                        name: d.name.clone(),
                        display: display_name(d),
                        texture: d.texture.clone(),
                        char_def: d.def.clone(),
                        hazeburnt: is_hazeburnt(d),
                        vanilla_area: vanilla_area(d),
                        disabled: app.monster_disabled.contains(&d.name),
                        group_label,
                        group_order,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let mut grouped: std::collections::BTreeMap<(usize, String), Vec<usize>> =
        std::collections::BTreeMap::new();
    for (i, entry) in entries.iter().enumerate() {
        grouped
            .entry((entry.group_order, entry.group_label.clone()))
            .or_default()
            .push(i);
    }

    let selected_count = app
        .hazeburnt_spawns
        .get(&area)
        .map(|names| names.len())
        .unwrap_or(0);
    // Selection row: Enable/Disable act on the selected tiles (multi-selection or the last click).
    let selection: Vec<usize> = if !app.hazeburnt_selected_multi.is_empty() {
        let mut v: Vec<usize> = app.hazeburnt_selected_multi.iter().copied().collect();
        v.sort_unstable();
        v
    } else {
        app.hazeburnt_selected_single.into_iter().collect()
    };
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let has_selection = !selection.is_empty();
        if ui
            .add_enabled(has_selection, egui::Button::new("Enable"))
            .on_hover_text("Add every selected monster to this area's spawn pool")
            .clicked()
        {
            for &i in &selection {
                if let Some(entry) = entries.get(i) {
                    app.hazeburnt_spawns
                        .entry(area)
                        .or_default()
                        .insert(entry.name.clone());
                }
            }
        }
        if ui
            .add_enabled(has_selection, egui::Button::new("Disable"))
            .on_hover_text("Remove every selected monster from this area's spawn pool")
            .clicked()
        {
            for &i in &selection {
                if let Some(entry) = entries.get(i) {
                    if let Some(pool) = app.hazeburnt_spawns.get_mut(&area) {
                        pool.remove(&entry.name);
                    }
                }
            }
        }
        ui.label(
            egui::RichText::new(format!(
                "{} monster(s) selected{}",
                selection.len(),
                if show_all {
                    ""
                } else {
                    " - showing hazeburnt variants only"
                }
            ))
            .color(egui::Color32::GRAY),
        );
        ui.label(
            egui::RichText::new(format!("({} in this pool)", selected_count))
                .color(egui::Color32::GRAY),
        );
    });
    ui.separator();

    let mut gsel = std::mem::take(&mut app.hazeburnt_grid_sel);
    gsel.begin(ui);
    gsel.display_order.clear();
    for indices in grouped.values() {
        for &i in indices {
            gsel.display_order.push(i);
        }
    }

    egui::ScrollArea::both()
        .scroll_source(crate::tabs::multisel::grid_scroll_source(ui))
        .auto_shrink([false; 2])
        .show_viewport(ui, |ui, viewport| {
            let metrics = GridMetrics::new(
                ui,
                app.config.item_icon_size,
                app.config.grid_font_size,
                app.config.category_font_size,
                8.0,
                8.0,
            );
            let overscan = 3.0 * (metrics.icon_size + metrics.pad_x);
            let vp_min = viewport.min.x - overscan;
            let vp_max = viewport.max.x + overscan;

            for ((_, cat), indices) in &grouped {
                ui.style_mut().interaction.selectable_labels = false;
                ui.label(
                    egui::RichText::new(cat)
                        .strong()
                        .size(metrics.category_font_size),
                );

                egui::Grid::new(("hazeburnt_spawns", cat))
                    .spacing([metrics.spacing_x, metrics.spacing_y])
                    .show(ui, |ui| {
                        let mut x = 0.0f32;
                        for &i in indices {
                            let Some(entry) = entries.get(i) else { continue };
                            let selected = app
                                .hazeburnt_spawns
                                .get(&area)
                                .map(|names| names.contains(&entry.name))
                                .unwrap_or(false);
                            let note = entry_note(entry, area);
                            let item_size = metrics.tile_size(
                                &entry.display,
                                !entry.texture.is_empty(),
                                if note.is_some() { 1 } else { 0 },
                            );
                            let start = x;
                            let end = x + item_size.x;
                            x = end + metrics.spacing_x;
                            if end < vp_min || start > vp_max {
                                ui.allocate_space(item_size);
                                continue;
                            }

                            let tex = if entry.texture.is_empty() {
                                None
                            } else {
                                app.monster_texture_cache.get_or_assemble(
                                    ui.ctx(),
                                    &entry.char_def,
                                    &entry.texture,
                                )
                            };
                            let response = ui
                                .vertical(|ui| {
                                    monster_grid::monster_tile(
                                        ui,
                                        tex.as_ref(),
                                        &entry.display,
                                        &metrics,
                                        selected,
                                        note.as_ref().map(|(t, c)| (t.as_str(), *c)),
                                    )
                                })
                                .inner;
                            gsel.cell(response.rect, i);
                            let ui_selected = app.hazeburnt_selected_multi.contains(&i)
                                || app.hazeburnt_selected_single == Some(i)
                                || gsel.is_box_hit(&i);
                            // Green border: the monster is in this area's pool.
                            crate::tabs::multisel::paint_sel_outline(
                                ui,
                                response.rect,
                                selected,
                            );
                            // Orange border: the tile is selected in the UI (Enable/Disable target).
                            crate::tabs::multisel::paint_outline_colored(
                                ui,
                                response.rect,
                                ui_selected.then_some(egui::Color32::from_rgb(230, 150, 60)),
                            );

                            let hover = format!(
                                "{}\nClick to {} Area {}'s spawn pool{}",
                                entry.display,
                                if selected { "remove from" } else { "add to" },
                                area,
                                if entry.hazeburnt {
                                    ""
                                } else {
                                    "\nNot hazeburnt in vanilla: spawning it from a pool gives it the hazeburnt behavior."
                                }
                            );
                            if response.on_hover_text(hover).clicked()
                                && app.config.hazeburnt_toggle_on_click
                            {
                                let set = app.hazeburnt_spawns.entry(area).or_default();
                                if selected {
                                    set.remove(&entry.name);
                                } else {
                                    set.insert(entry.name.clone());
                                }
                            }
                        }
                    });

                ui.add_space(8.0);
            }

            gsel.update_target();
            gsel.paint(ui);
            gsel.end(
                ui,
                &mut app.hazeburnt_selected_multi,
                &mut app.hazeburnt_selected_single,
            );
        });
    app.hazeburnt_grid_sel = gsel;
}
