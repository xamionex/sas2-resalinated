use crate::atlas::{ItemAtlas, MonsterTextureCache};
use crate::catalog::{load_loot_catalog, load_monster_catalog};
use crate::config::{
    default_drag_sensitivity, default_item_font_size, default_item_icon_size, ResalinatedConfig,
};
use crate::magic_slot::MagicSlotOverrides;
use crate::preset::{PresetManager, PresetMeta};
use crate::tabs::{items, manager, monsters, preset_info, Tab};
use eframe::egui;
use rfd::FileDialog;
use sas2_parser::loot_catalog::LootCatalog;
use sas2_parser::monster_catalog::MonsterCatalog;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ResalinatedApp {
    pub config: ResalinatedConfig,
    pub vanilla_data: Option<Vec<u8>>,
    pub vanilla_catalog: Option<LootCatalog>,
    pub working_catalog: Option<LootCatalog>,
    pub game_path: Option<PathBuf>,
    pub preset_manager: PresetManager,
    pub active_tab: Tab,
    // Items tab
    pub selected_item_idx: Option<usize>,
    pub search_filter: String,
    pub magic_slot_overrides: HashMap<String, HashMap<i32, MagicSlotOverrides>>,
    // Preset Info tab
    pub edit_folder_name: String,
    pub edit_meta: PresetMeta,
    // Manager tab
    pub manager_selected_available: Option<usize>,
    pub manager_selected_enabled: Option<usize>,
    pub error_message: Option<String>,
    pub confirm_overwrite_folder: Option<String>,
    pub show_only_changed_items: bool,
    pub apply_feedback_time: Option<std::time::Instant>,
    // Monsters tab
    pub vanilla_monster_catalog: Option<MonsterCatalog>,
    pub working_monster_catalog: Option<MonsterCatalog>,
    pub selected_monster_idx: Option<usize>,
    pub monster_search_filter: String,
    pub show_only_changed_monsters: bool,
    pub item_atlas: Option<ItemAtlas>,
    pub monster_texture_cache: MonsterTextureCache,
    pub settings_open: bool,
    pub catalog_error: Option<String>,
    pub monster_catalog_error: Option<String>,
    pub config_save_timer: f32,
    pub magic_item_picker_open: bool,
    pub magic_item_search: String,
    pub magic_item_picker_target_slot_id: Option<i32>,
}

impl Default for ResalinatedApp {
    fn default() -> Self {
        let config = ResalinatedConfig::load();
        let game_path = config.game_path.clone();
        let mut app = Self {
            config,
            vanilla_data: None,
            vanilla_catalog: None,
            working_catalog: None,
            game_path,
            preset_manager: PresetManager::new(),
            active_tab: Tab::Manager,
            selected_item_idx: None,
            search_filter: String::new(),
            magic_slot_overrides: HashMap::new(),
            edit_folder_name: String::new(),
            edit_meta: PresetMeta {
                name: String::new(),
                version: "1.0.0".to_string(),
                author: String::new(),
                description: String::new(),
            },
            manager_selected_available: None,
            manager_selected_enabled: None,
            error_message: None,
            confirm_overwrite_folder: None,
            show_only_changed_items: false,
            apply_feedback_time: None,
            vanilla_monster_catalog: None,
            working_monster_catalog: None,
            selected_monster_idx: None,
            monster_search_filter: String::new(),
            show_only_changed_monsters: false,
            item_atlas: None,
            monster_texture_cache: MonsterTextureCache::new(),
            settings_open: false,
            catalog_error: None,
            monster_catalog_error: None,
            config_save_timer: 0.0,
            magic_item_picker_open: false,
            magic_item_search: String::new(),
            magic_item_picker_target_slot_id: None,
        };

        // Load catalogs immediately if we already have a game path stored
        // Textures load in app.ui()
        if let Some(game_path) = &app.config.game_path.clone() {
            app.load_catalogs(game_path);
        }
        app
    }
}

impl ResalinatedApp {
    /// Load (or reload) all three catalogs from `game_path`.
    fn load_catalogs(&mut self, game_path: &Path) {
        self.preset_manager.set_game_path(&game_path);

        match load_loot_catalog(game_path) {
            Ok(cat) => {
                self.vanilla_catalog = Some(cat.clone());
                self.working_catalog = Some(cat.clone());
                self.catalog_error = None;
            }
            Err(e) => {
                self.vanilla_catalog = None;
                self.working_catalog = None;
                self.catalog_error = Some(e);
            }
        }
        match load_monster_catalog(game_path) {
            Ok(cat) => {
                self.vanilla_monster_catalog = Some(cat.clone());
                self.working_monster_catalog = Some(cat.clone());
                self.monster_catalog_error = None;

                // start background texture loading
                let names: Vec<String> = cat
                    .monsters
                    .iter()
                    .filter(|m| !m.texture.is_empty())
                    .map(|m| m.texture.clone())
                    .collect();
                self.monster_texture_cache.set_game_path(game_path);
                self.monster_texture_cache.start_preload(game_path, names);
            }
            Err(e) => {
                self.vanilla_monster_catalog = None;
                self.working_monster_catalog = None;
                self.monster_catalog_error = Some(e);
            }
        }
    }

    /// Update the stored game path, persist it, and reload everything.
    pub fn set_game_path(&mut self, path: PathBuf) {
        self.config.game_path = Some(path.clone());
        self.config.save();

        self.load_catalogs(&path);

        // Drop the old atlas and texture so they get re-loaded lazily on the next frame that needs them.
        self.item_atlas = None;
    }

    pub fn choose_game_folder(&mut self) {
        if let Some(folder) = FileDialog::new().pick_folder() {
            self.set_game_path(folder);
        }
    }

    /// Merge all enabled presets (starting from vanilla) and return the final catalog bytes.
    pub(crate) fn merge_enabled_presets(&self) -> Result<Vec<u8>, String> {
        let vanilla = self
            .vanilla_catalog
            .as_ref()
            .ok_or("No vanilla catalog loaded")?;
        let mut merged = vanilla.clone(); // now works: LootCatalog is Clone

        for folder_name in self.preset_manager.enabled_presets() {
            if folder_name == "Vanilla (Base)" {
                continue;
            }
            if let Some(preset_data) = self.preset_manager.get_preset_loot(folder_name) {
                let preset = LootCatalog::load_from_bytes(&preset_data)
                    .map_err(|e| format!("Failed to parse preset '{}': {}", folder_name, e))?;
                for def in preset.loot_defs {
                    if let Some(existing) = merged.loot_defs.iter_mut().find(|d| d.name == def.name)
                    {
                        *existing = def;
                    } else {
                        merged.loot_defs.push(def);
                    }
                }
            }
        }

        // Rebuild by_name map
        merged.by_name.clear();
        for (i, def) in merged.loot_defs.iter().enumerate() {
            merged.by_name.insert(def.name.clone(), i);
        }

        merged.black_starstone_index = merged
            .loot_defs
            .iter()
            .position(|d| d.name == "black_pearl")
            .or_else(|| {
                merged
                    .loot_defs
                    .iter()
                    .position(|d| d.title.iter().any(|t| t.contains("Black Starstone")))
            });
        merged.gray_starstone_index = merged
            .loot_defs
            .iter()
            .position(|d| d.name == "gray_pearl")
            .or_else(|| {
                merged
                    .loot_defs
                    .iter()
                    .position(|d| d.title.iter().any(|t| t.contains("Gray Starstone")))
            });

        merged
            .to_bytes()
            .map_err(|e| format!("Serialization error: {}", e))
    }

    /// Merge magic overrides from all enabled presets (later ones override earlier ones) and return the resulting JSON string for magic_damage.json.
    fn merged_magic_damage_json(&self) -> Result<String, String> {
        let mut merged: HashMap<String, HashMap<i32, MagicSlotOverrides>> = HashMap::new();

        for folder_name in self.preset_manager.enabled_presets() {
            if folder_name == "Vanilla (Base)" {
                continue;
            }
            if let Some(data) = self
                .preset_manager
                .get_preset_file(folder_name, "magic_overrides.json")
            {
                let overrides: HashMap<String, HashMap<i32, MagicSlotOverrides>> =
                    serde_json::from_slice(&data).map_err(|e| {
                        format!("Invalid magic_overrides.json in '{}': {}", folder_name, e)
                    })?;
                for (weapon, slots) in overrides {
                    let entry = merged.entry(weapon).or_default();
                    for (slot_id, over) in slots {
                        entry.insert(slot_id, over);
                    }
                }
            }
        }

        // Convert to the format expected by MagicDamagePatch
        let mut output = serde_json::Map::new();
        for (weapon, slots) in merged {
            let mut weapon_entry = serde_json::Map::new();
            for (slot_id, over) in slots {
                let label = match slot_id {
                    14 => "x",
                    15 => "y",
                    16 => "b",
                    _ => continue,
                };
                weapon_entry.insert(label.to_string(), serde_json::json!(over.damage));
            }
            if !weapon_entry.is_empty() {
                output.insert(weapon, serde_json::Value::Object(weapon_entry));
            }
        }

        serde_json::to_string_pretty(&output).map_err(|e| format!("Serialization error: {}", e))
    }

    /// Create a delta catalog containing only items that differ from vanilla.
    pub(crate) fn build_delta_catalog(&self) -> Result<Vec<u8>, String> {
        let vanilla = self.vanilla_catalog.as_ref().ok_or("No vanilla catalog")?;
        let working = self.working_catalog.as_ref().ok_or("No working catalog")?;

        let mut delta = LootCatalog {
            loot_defs: Vec::new(),
            by_name: HashMap::new(),
            black_starstone_index: None,
            gray_starstone_index: None,
        };

        for def in &working.loot_defs {
            let is_new_or_modified = match vanilla.loot_defs.iter().find(|vd| vd.name == def.name) {
                Some(vdef) => def.to_bytes().ok() != vdef.to_bytes().ok(),
                None => true,
            };
            if is_new_or_modified {
                delta.loot_defs.push(def.clone());
            }
        }

        delta
            .to_bytes()
            .map_err(|e| format!("Delta serialization error: {}", e))
    }

    // Merge monsters enabled presets
    pub(crate) fn merge_enabled_monster_presets(&self) -> Result<Vec<u8>, String> {
        let vanilla = self
            .vanilla_monster_catalog
            .as_ref()
            .ok_or("No vanilla monster catalog")?;
        let mut merged = vanilla.clone();
        for folder_name in self.preset_manager.enabled_presets() {
            if folder_name == "Vanilla (Base)" {
                continue;
            }
            if let Some(data) = self
                .preset_manager
                .get_preset_file(folder_name, "monsters.zms")
            {
                let preset = MonsterCatalog::load_from_bytes(&data).map_err(|e| {
                    format!("Failed to parse monster preset '{}': {}", folder_name, e)
                })?;
                for def in preset.monsters {
                    if let Some(existing) = merged.monsters.iter_mut().find(|d| d.name == def.name)
                    {
                        *existing = def;
                    } else {
                        merged.monsters.push(def);
                    }
                }
            }
        }
        merged.by_name.clear();
        for (i, def) in merged.monsters.iter().enumerate() {
            merged.by_name.insert(def.name.clone(), i as i32);
        }
        merged
            .to_bytes()
            .map_err(|e| format!("Serialization error: {}", e))
    }

    // Build delta monster catalog
    pub(crate) fn build_delta_monster_catalog(&self) -> Result<Vec<u8>, String> {
        let vanilla = self
            .vanilla_monster_catalog
            .as_ref()
            .ok_or("No vanilla monster catalog")?;
        let working = self
            .working_monster_catalog
            .as_ref()
            .ok_or("No working monster catalog")?;
        let mut delta = MonsterCatalog {
            monsters: Vec::new(),
            by_name: HashMap::new(),
        };
        for def in &working.monsters {
            let is_new_or_modified = match vanilla.monsters.iter().find(|vd| vd.name == def.name) {
                Some(vdef) => def.to_bytes().ok() != vdef.to_bytes().ok(),
                None => true,
            };
            if is_new_or_modified {
                delta.monsters.push(def.clone());
            }
        }
        delta
            .to_bytes()
            .map_err(|e| format!("Delta serialization error: {}", e))
    }

    pub fn save_preset(&mut self, folder_name: &str, meta: PresetMeta) -> Result<(), String> {
        let loot_delta = self.build_delta_catalog()?;
        self.preset_manager
            .save_preset_loot(folder_name, &loot_delta)?;
        if let Some(_) = &self.working_monster_catalog {
            let monster_delta = self.build_delta_monster_catalog()?;
            self.preset_manager
                .save_preset_file(folder_name, "monsters.zms", &monster_delta)?;
        }
        let magic_bytes = serde_json::to_vec(&self.magic_slot_overrides)
            .map_err(|e| format!("Failed to serialize magic overrides: {}", e))?;
        self.preset_manager
            .save_preset_file(folder_name, "magic_overrides.json", &magic_bytes)?;
        self.preset_manager.save_preset_meta(folder_name, &meta)?;
        self.preset_manager.refresh();
        Ok(())
    }

    pub(crate) fn apply_enabled_presets(&mut self) {
        // Loot
        match self.merge_enabled_presets() {
            Ok(merged_loot) => {
                if let Some(gp) = &self.game_path {
                    let dest = gp.join("BepInEx/config/amione.SaS2Resalter/loot.zls");
                    if let Err(e) = std::fs::create_dir_all(dest.parent().unwrap()) {
                        self.error_message = Some(format!("Failed to create directory: {}", e));
                    } else if let Err(e) = std::fs::write(&dest, &merged_loot) {
                        self.error_message = Some(format!("Failed to write loot.zls: {}", e));
                    } else {
                        if let Ok(cat) = LootCatalog::load_from_bytes(&merged_loot) {
                            self.working_catalog = Some(cat);
                            self.error_message = None;
                        } else {
                            self.error_message =
                                Some("Failed to parse merged loot.zls".to_string());
                        }
                    }
                }
            }
            Err(e) => self.error_message = Some(e),
        }

        // Monsters
        match self.merge_enabled_monster_presets() {
            Ok(merged_monsters) => {
                if let Some(gp) = &self.game_path {
                    let dest = gp.join("BepInEx/config/amione.SaS2Resalter/monsters.zms");
                    if let Err(e) = std::fs::create_dir_all(dest.parent().unwrap()) {
                        self.error_message = Some(format!("Failed to create directory: {}", e));
                    } else if let Err(e) = std::fs::write(&dest, &merged_monsters) {
                        self.error_message = Some(format!("Failed to write monsters.zms: {}", e));
                    } else {
                        if let Ok(cat) = MonsterCatalog::load_from_bytes(&merged_monsters) {
                            self.working_monster_catalog = Some(cat);
                            self.error_message = None;
                        } else {
                            self.error_message =
                                Some("Failed to parse merged monsters.zms".to_string());
                        }
                    }
                }
            }
            Err(e) => self.error_message = Some(e),
        }

        // Write magic_damage.json
        match self.merged_magic_damage_json() {
            Ok(json) => {
                if let Some(gp) = &self.game_path {
                    let config_dir = gp.join("BepInEx/config/amione.SaS2Resalter");
                    if let Err(e) = std::fs::create_dir_all(&config_dir) {
                        self.error_message = Some(format!("Failed to create config dir: {}", e));
                    } else {
                        let config_path = config_dir.join("magic_damage.json");
                        if let Err(e) = std::fs::write(&config_path, json) {
                            self.error_message =
                                Some(format!("Failed to write magic_damage.json: {}", e));
                        }
                    }
                }
            }
            Err(e) => self.error_message = Some(e),
        }
    }

    /// Load a preset folder into the working catalog by merging it on top of vanilla.
    /// This keeps all vanilla items and only changes the ones the preset modifies.
    pub fn load_preset(&mut self, folder_name: &str) {
        if let Some(data) = self.preset_manager.get_preset_loot(folder_name) {
            match LootCatalog::load_from_bytes(&data) {
                Ok(preset_cat) => {
                    // Start with a copy of the vanilla catalog
                    let vanilla = match &self.vanilla_catalog {
                        Some(v) => v.clone(),
                        None => {
                            self.error_message = Some("Vanilla catalog not loaded".to_string());
                            return;
                        }
                    };
                    let mut merged = vanilla;

                    // Overlay the presets items
                    for def in preset_cat.loot_defs {
                        if let Some(existing) =
                            merged.loot_defs.iter_mut().find(|d| d.name == def.name)
                        {
                            *existing = def; // replace modified item
                        } else {
                            merged.loot_defs.push(def); // new item
                        }
                    }

                    // Load magic overrides (if the preset contains them), otherwise start with empty map.
                    if let Some(data) = self
                        .preset_manager
                        .get_preset_file(folder_name, "magic_overrides.json")
                    {
                        match serde_json::from_slice(&data) {
                            Ok(map) => self.magic_slot_overrides = map,
                            Err(e) => {
                                self.magic_slot_overrides.clear();
                                self.error_message =
                                    Some(format!("Failed to load magic overrides: {}", e));
                            }
                        }
                    } else {
                        self.magic_slot_overrides.clear();
                    }

                    // Rebuild the by_name map
                    merged.by_name.clear();
                    for (i, def) in merged.loot_defs.iter().enumerate() {
                        merged.by_name.insert(def.name.clone(), i);
                    }

                    // Recalculate starstone indices
                    merged.black_starstone_index = merged
                        .loot_defs
                        .iter()
                        .position(|d| d.name == "black_pearl")
                        .or_else(|| {
                            merged
                                .loot_defs
                                .iter()
                                .position(|d| d.title.iter().any(|t| t.contains("Black Starstone")))
                        });
                    merged.gray_starstone_index = merged
                        .loot_defs
                        .iter()
                        .position(|d| d.name == "gray_pearl")
                        .or_else(|| {
                            merged
                                .loot_defs
                                .iter()
                                .position(|d| d.title.iter().any(|t| t.contains("Gray Starstone")))
                        });

                    self.working_catalog = Some(merged);

                    // Also load the presets metadata
                    if let Some(p) = self
                        .preset_manager
                        .installed_presets()
                        .iter()
                        .find(|p| p.folder_name == folder_name)
                    {
                        self.edit_meta = p.meta.clone();
                        self.edit_folder_name = p.folder_name.clone();
                    }
                    self.error_message = None;
                    self.active_tab = Tab::PresetInfo;
                }
                Err(e) => self.error_message = Some(format!("Failed to parse preset: {}", e)),
            }
        } else {
            self.error_message = Some("Preset loot data not found".to_string());
        }

        // Load monster data
        if let Some(data) = self
            .preset_manager
            .get_preset_file(folder_name, "monsters.zms")
        {
            match MonsterCatalog::load_from_bytes(&data) {
                Ok(preset_cat) => {
                    let vanilla = match &self.vanilla_monster_catalog {
                        Some(v) => v.clone(),
                        None => {
                            return;
                        } // just skip if no vanilla monsters
                    };
                    let mut merged = vanilla;
                    for def in preset_cat.monsters {
                        if let Some(existing) =
                            merged.monsters.iter_mut().find(|d| d.name == def.name)
                        {
                            *existing = def;
                        } else {
                            merged.monsters.push(def);
                        }
                    }
                    merged.by_name.clear();
                    for (i, def) in merged.monsters.iter().enumerate() {
                        merged.by_name.insert(def.name.clone(), i as i32);
                    }
                    self.working_monster_catalog = Some(merged);
                }
                Err(e) => {
                    self.error_message = Some(format!("Failed to parse monster preset: {}", e))
                }
            }
        }

        // Load metadata
        if let Some(p) = self
            .preset_manager
            .installed_presets()
            .iter()
            .find(|p| p.folder_name == folder_name)
        {
            self.edit_meta = p.meta.clone();
            self.edit_folder_name = p.folder_name.clone();
        }
        self.active_tab = Tab::PresetInfo;
    }

    pub fn show_settings_window(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }

        let mut is_open = self.settings_open;

        egui::Window::new("Configure UI")
            .open(&mut is_open)
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                ui.vertical(|ui| {
                    ui.heading("Item Display Settings");

                    ui.horizontal(|ui| {
                        ui.label("Item Icon Size:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.config.item_icon_size)
                                    .range(32.0..=128.0)
                                    .speed(self.config.drag_value_sensitivity)
                                    .suffix("px"),
                            )
                            .changed()
                        {
                            self.config_save_timer = 0.1;
                        }
                        if ui.button("Reset").clicked() {
                            self.config.item_icon_size = default_item_icon_size();
                            self.config_save_timer = 0.1;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Item Font Size:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.config.item_font_size)
                                    .range(6.0..=24.0)
                                    .speed(self.config.drag_value_sensitivity)
                                    .suffix("pt"),
                            )
                            .changed()
                        {
                            self.config_save_timer = 0.1;
                        }
                        if ui.button("Reset").clicked() {
                            self.config.item_font_size = default_item_font_size();
                            self.config_save_timer = 0.1;
                        }
                    });

                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label("Drag Value Sensitivity:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.config.drag_value_sensitivity)
                                    .range(0.005..=1.0)
                                    .speed(0.025)
                                    .suffix("x"),
                            )
                            .changed()
                        {
                            self.config_save_timer = 0.1;
                        }
                        if ui.button("Reset").clicked() {
                            self.config.drag_value_sensitivity = default_drag_sensitivity();
                            self.config_save_timer = 0.1;
                        }
                    });

                    ui.horizontal(|ui| {
                        ui.label("Test Drag Value Sensitivity:");
                        if ui
                            .add(
                                egui::DragValue::new(&mut self.config.dummy_drag_value)
                                    .range(0.0..=1000.0)
                                    .speed(self.config.drag_value_sensitivity)
                                    .suffix("x"),
                            )
                            .changed()
                        {
                            self.config_save_timer = 0.1;
                        }
                    });
                });
            });

        self.settings_open = is_open;
    }

    /// Serialize the magic_slot_overrides map to a JSON byte vector.
    pub fn save_magic_overrides_to_bytes(&self) -> Result<Vec<u8>, String> {
        let json = serde_json::to_vec(&self.magic_slot_overrides)
            .map_err(|e| format!("Failed to serialize magic overrides: {}", e))?;
        Ok(json)
    }

    /// Deserialize the magic_slot_overrides map from bytes.
    pub fn load_magic_overrides_from_bytes(&mut self, data: &[u8]) -> Result<(), String> {
        let overrides: HashMap<String, HashMap<i32, MagicSlotOverrides>> =
            serde_json::from_slice(data)
                .map_err(|e| format!("Failed to deserialize magic overrides: {}", e))?;
        self.magic_slot_overrides = overrides;
        Ok(())
    }
}

impl eframe::App for ResalinatedApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if self.item_atlas.is_none() {
            if let Some(game_path) = self.config.game_path.clone() {
                match ItemAtlas::load(&game_path, ui.ctx()) {
                    Ok(atlas) => self.item_atlas = Some(atlas),
                    Err(e) => eprintln!("Failed to load item atlas: {}", e),
                }
            }
        }

        egui::CentralPanel::default().show_inside(ui, |ui| {
            // Menu bar
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Set Game Folder").clicked() {
                        self.choose_game_folder();
                        ui.close();
                    }
                    if ui.button("Open Presets Folder").clicked() {
                        let presets_dir = self.preset_manager.presets_dir();  // expose via a public getter
                        if let Err(e) = open::that(&presets_dir) {
                            self.error_message = Some(format!("Failed to open presets folder: {}", e));
                        }
                    }
                });
                ui.menu_button("Settings", |ui| {
                    if ui.button("Configure UI").clicked() {
                        self.settings_open = true;
                        ui.close();
                    }
                });
            });

            self.show_settings_window(ui.ctx());

            // Game folder status line
            if let Some(game_path) = &self.config.game_path {
                ui.label(format!("Game folder: {}", game_path.display()));
            } else {
                ui.colored_label(egui::Color32::YELLOW, "Game folder not set (needed for item names/icons, and bestiary textures/names)", );
                if ui.button("Set Game Folder").clicked() {
                    self.choose_game_folder();
                }
            }
            if let Some(err) = &self.catalog_error {
                ui.colored_label(egui::Color32::RED, format!("Loot catalog error: {}", err));
            }
            if let Some(err) = &self.monster_catalog_error {
                ui.colored_label(egui::Color32::RED, format!("Monster catalog error: {}", err));
            }
            if let Some(err) = &self.error_message { ui.colored_label(egui::Color32::RED, err); }

            ui.separator();

            // Tab bar
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.active_tab, Tab::Manager, "Manager");
                ui.selectable_value(&mut self.active_tab, Tab::PresetInfo, "Preset Info");
                ui.selectable_value(&mut self.active_tab, Tab::Items, "Items");
                ui.selectable_value(&mut self.active_tab, Tab::Monsters, "Monsters");

                ui.vertical(|ui| {
                    // progress bar while textures are loading
                    if let Some((loaded, total)) = self.monster_texture_cache.progress() {
                        let fraction = loaded as f32 / total as f32;
                        ui.add(egui::ProgressBar::new(fraction.min(1.0)).show_percentage());
                        ui.label(format!("{}/{} textures loaded…", loaded, total));
                    }
                });
            });

            ui.separator();

            // Content
            match self.active_tab {
                Tab::Manager => manager::show(self, ui),
                Tab::PresetInfo => preset_info::show(self, ui),
                Tab::Items => items::show(self, ui),
                Tab::Monsters => monsters::show(self, ui),
            }
        });
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        //if self.active_tab == Tab::Monsters {
        self.monster_texture_cache.update(ctx);
        if self.monster_texture_cache.is_loading() {
            ctx.request_repaint();
        }

        if self.config_save_timer > 0.0 {
            self.config_save_timer -= ctx.input(|i| i.stable_dt);

            if self.config_save_timer <= 0.01 {
                self.config.save();
                eprintln!("Config saved.");
                self.config_save_timer = 0.0;
            }
        }

        //}
    }
}
