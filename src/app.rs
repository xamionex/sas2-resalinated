use crate::atlas::{ItemAtlas, MonsterTextureCache};
use crate::catalog::{load_loot_catalog, load_monster_catalog};
use crate::config::{
    default_drag_sensitivity, default_item_font_size, default_item_icon_size, ResalinatedConfig,
};
use crate::magic_slot::MagicSlotOverrides;
use crate::preset::{PresetManager, PresetMeta};
use crate::tabs::{animations, images, items, manager, monsters, preset_info, textures, Tab};
use eframe::egui;
use rfd::FileDialog;
use sas2_parser::loot_catalog::LootCatalog;
use sas2_parser::monster_catalog::MonsterCatalog;
use sas2_parser::subflags::SubFlagDefCatalog;
use sas2_parser::xtexture::MasterTextures;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Serialize a loot catalog excluding the named (disabled) items, reindexing afterwards.
fn loot_bytes_excluding(cat: &LootCatalog, disabled: &HashSet<String>) -> Result<Vec<u8>, String> {
    if disabled.is_empty() {
        return cat
            .to_bytes()
            .map_err(|e| format!("Serialization error: {}", e));
    }
    let mut c = cat.clone();
    c.loot_defs.retain(|d| !disabled.contains(&d.name));
    c.by_name.clear();
    for (i, d) in c.loot_defs.iter().enumerate() {
        c.by_name.insert(d.name.clone(), i);
    }
    c.black_starstone_index = c.loot_defs.iter().position(|d| d.name == "black_pearl");
    c.gray_starstone_index = c.loot_defs.iter().position(|d| d.name == "gray_pearl");
    c.to_bytes()
        .map_err(|e| format!("Serialization error: {}", e))
}

/// Serialize a monster catalog excluding the named (disabled) monsters, reindexing afterwards.
fn monster_bytes_excluding(
    cat: &MonsterCatalog,
    disabled: &HashSet<String>,
) -> Result<Vec<u8>, String> {
    if disabled.is_empty() {
        return cat
            .to_bytes()
            .map_err(|e| format!("Serialization error: {}", e));
    }
    let mut c = cat.clone();
    c.monsters.retain(|d| !disabled.contains(&d.name));
    c.by_name.clear();
    for (i, d) in c.monsters.iter().enumerate() {
        c.by_name.insert(d.name.clone(), i as i32);
    }
    c.to_bytes()
        .map_err(|e| format!("Serialization error: {}", e))
}

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
    /// Items the user marked as sold in shops: item name -> optional flag requirement
    /// (empty string = always for sale). Written to shop_additions.txt on apply.
    pub shop_additions: HashMap<String, String>,
    /// Items the user marked as sold in the craft/equipment menu (item name -> optional flag).
    /// Independent of shop_additions. Written to craft_additions.txt on apply.
    pub craft_additions: HashMap<String, String>,
    /// Disabled loot/monster names. Disabled entries stay in the working catalog (so they can be
    /// re-enabled) but are excluded from the exported catalog the game loads.
    pub loot_disabled: HashSet<String>,
    pub monster_disabled: HashSet<String>,
    // Preset Info tab
    pub edit_folder_name: String,
    pub edit_meta: PresetMeta,
    /// When true the folder name field is editable (manual override, not recommended).
    pub folder_override_enabled: bool,
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
    /// Focus the search field on the first frame a picker window opens.
    pub magic_item_picker_focus: bool,
    /// "Copy logic from" picker (shared by the Items and Monsters tabs; only the active tab renders it).
    pub copy_picker_open: bool,
    pub copy_picker_search: String,
    pub copy_picker_focus: bool,
    /// Icon pickers (Items tab): vanilla game icons and custom icons, each searchable.
    pub custom_icon_picker_open: bool,
    pub custom_icon_search: String,
    pub custom_icon_picker_focus: bool,
    pub vanilla_icon_picker_open: bool,
    pub vanilla_icon_search: String,
    pub vanilla_icon_picker_focus: bool,
    pub texture_editor: crate::texture_editor::TextureEditor,
    pub anim_editor: crate::anim_editor::AnimEditor,
    pub image_editor: crate::image_editor::ImageEditor,
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
            shop_additions: HashMap::new(),
            craft_additions: HashMap::new(),
            loot_disabled: HashSet::new(),
            monster_disabled: HashSet::new(),
            edit_folder_name: String::new(),
            edit_meta: PresetMeta {
                name: String::new(),
                version: "1.0.0".to_string(),
                author: String::new(),
                description: String::new(),
                editor_version: None,
                folder_override: false,
            },
            folder_override_enabled: false,
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
            magic_item_picker_focus: false,
            copy_picker_open: false,
            copy_picker_search: String::new(),
            copy_picker_focus: false,
            custom_icon_picker_open: false,
            custom_icon_search: String::new(),
            custom_icon_picker_focus: false,
            vanilla_icon_picker_open: false,
            vanilla_icon_search: String::new(),
            vanilla_icon_picker_focus: false,
            texture_editor: crate::texture_editor::TextureEditor::default(),
            anim_editor: crate::anim_editor::AnimEditor::default(),
            image_editor: crate::image_editor::ImageEditor::default(),
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
        self.texture_editor = crate::texture_editor::TextureEditor::default();
        self.anim_editor = crate::anim_editor::AnimEditor::default();
        self.image_editor = crate::image_editor::ImageEditor::default();
    }

    pub fn choose_game_folder(&mut self) {
        if let Some(folder) = FileDialog::new().pick_folder() {
            self.set_game_path(folder);
        }
    }

    /// Snapshot the working-assets folder into a preset's `assets/` folder (replacing it).
    fn save_assets_to_preset(&self, folder_name: &str) -> Result<(), String> {
        let working = crate::assets::working_root();
        let dst = crate::assets::preset_assets_root(
            &self.preset_manager.presets_dir().join(folder_name),
        );
        crate::assets::remove_dir_if_exists(&dst);
        crate::assets::copy_dir_all(&working, &dst)
            .map_err(|e| format!("Failed to copy assets into preset: {}", e))?;
        Ok(())
    }

    /// Replace the working-assets folder with a preset's assets, and reset the asset editors so
    /// they reload from it.
    fn load_assets_from_preset(&mut self, folder_name: &str) {
        let working = crate::assets::working_root();
        crate::assets::remove_dir_if_exists(&working);
        let src = crate::assets::preset_assets_root(
            &self.preset_manager.presets_dir().join(folder_name),
        );
        let _ = crate::assets::copy_dir_all(&src, &working);
        self.reset_asset_editors();
    }

    fn reset_asset_editors(&mut self) {
        self.texture_editor = crate::texture_editor::TextureEditor::default();
        self.anim_editor = crate::anim_editor::AnimEditor::default();
        self.image_editor = crate::image_editor::ImageEditor::default();
    }

    /// Merge every enabled preset's assets into the live config the loader reads. Disabled presets'
    /// assets are dropped (the config asset dirs are cleared first). Textures/char-defs/icons are
    /// copied last-wins; master.zcm is merged per entry against vanilla; the custom icon atlas is
    /// composited.
    fn merge_assets_to_config(&mut self) {
        let Some(gp) = self.game_path.clone() else {
            return;
        };
        let cfg = crate::assets::config_root(&gp);

        // Clear the asset areas so disabled presets stop contributing.
        crate::assets::remove_dir_if_exists(&cfg.join(crate::assets::TEXTURES_DIR));
        crate::assets::remove_dir_if_exists(&cfg.join("Character"));
        crate::assets::remove_dir_if_exists(&cfg.join(crate::assets::ICONS_DIR));
        let _ = std::fs::remove_file(cfg.join(crate::assets::MASTER_REL));

        let presets_dir = self.preset_manager.presets_dir().to_path_buf();
        let mut master_paths: Vec<PathBuf> = Vec::new();

        for folder in self.preset_manager.enabled_presets() {
            if folder == "Vanilla (Base)" {
                continue;
            }
            let pa = crate::assets::preset_assets_root(&presets_dir.join(folder));
            if !pa.exists() {
                continue;
            }
            let _ = crate::assets::copy_dir_all(&pa.join(crate::assets::TEXTURES_DIR),
                &cfg.join(crate::assets::TEXTURES_DIR));
            let _ = crate::assets::copy_dir_all(&pa.join("Character"), &cfg.join("Character"));
            let _ = crate::assets::copy_dir_all(&pa.join(crate::assets::ICONS_DIR),
                &cfg.join(crate::assets::ICONS_DIR));
            let m = pa.join(crate::assets::MASTER_REL);
            if m.exists() {
                master_paths.push(m);
            }
        }

        // master.zcm: overlay each preset's changed entries onto vanilla.
        if !master_paths.is_empty() {
            if let Some(bytes) = self.merge_master_zcm(&gp, &master_paths) {
                let dest = cfg.join(crate::assets::MASTER_REL);
                if let Some(parent) = dest.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if let Err(e) = std::fs::write(&dest, &bytes) {
                    self.error_message = Some(format!("Failed to write master.zcm: {}", e));
                }
            }
        }

        // Custom icon atlas from the merged icon set.
        let icons_dir = cfg.join(crate::assets::ICONS_DIR);
        if icons_dir.exists() {
            if let Err(e) = crate::image_editor::build_custom_atlas(
                &icons_dir,
                &cfg.join(crate::assets::CUSTOM_ITEMS_REL),
            ) {
                self.error_message = Some(e);
            }
        }
    }

    /// Per-entry master.zcm merge: start from vanilla, overlay any entry a preset changed.
    fn merge_master_zcm(&self, game_path: &Path, preset_masters: &[PathBuf]) -> Option<Vec<u8>> {
        let flagdefs_path = game_path.join("Content").join("gfx").join("flagdefs.zfd");
        let flag_defs = SubFlagDefCatalog::load_from_path(&flagdefs_path).ok()?;
        let vanilla_path = game_path.join("Content").join("gfx").join("master.zcm");
        let vanilla = MasterTextures::load_from_path(&vanilla_path, &flag_defs).ok()?;
        let mut merged = vanilla.clone();
        let mut changed = false;

        for pm_path in preset_masters {
            let Ok(pm) = MasterTextures::load_from_path(pm_path, &flag_defs) else {
                continue;
            };
            for (name_bytes, tex) in pm.entries {
                let name = String::from_utf8_lossy(&name_bytes).into_owned();
                if vanilla.get(&name) == Some(&tex) {
                    continue; // unchanged from vanilla, nothing to overlay
                }
                if let Some(existing) = merged
                    .entries
                    .iter_mut()
                    .find(|(n, _)| String::from_utf8_lossy(n) == name)
                {
                    existing.1 = tex;
                } else {
                    merged.entries.push((name_bytes, tex));
                }
                changed = true;
            }
        }

        if changed {
            merged.to_bytes(&flag_defs).ok()
        } else {
            None
        }
    }

    /// Disabled loot names from all enabled presets.
    fn collect_loot_disabled(&self) -> HashSet<String> {
        self.collect_disabled_names("loot_disabled.json")
    }

    /// Disabled monster names from all enabled presets.
    fn collect_monster_disabled(&self) -> HashSet<String> {
        self.collect_disabled_names("monster_disabled.json")
    }

    fn collect_disabled_names(&self, file: &str) -> HashSet<String> {
        let mut out = HashSet::new();
        for folder_name in self.preset_manager.enabled_presets() {
            if folder_name == "Vanilla (Base)" {
                continue;
            }
            if let Some(data) = self.preset_manager.get_preset_file(folder_name, file) {
                if let Ok(names) = serde_json::from_slice::<Vec<String>>(&data) {
                    out.extend(names);
                }
            }
        }
        out
    }

    /// Return a name not present in `used` by appending `_1`, `_2`, ... to `base`.
    fn unique_loot_name(used: &HashSet<String>, base: &str) -> String {
        let mut i = 1;
        loop {
            let candidate = format!("{}_{}", base, i);
            if !used.contains(&candidate) {
                return candidate;
            }
            i += 1;
        }
    }

    pub(crate) fn merge_enabled_presets(&self) -> Result<Vec<u8>, String> {
        let vanilla = self
            .vanilla_catalog
            .as_ref()
            .ok_or("No vanilla catalog loaded")?;
        let mut merged = vanilla.clone(); // now works: LootCatalog is Clone

        // Names that exist in vanilla. A preset def whose name matches one of these is a deliberate
        // modification of that vanilla item (replace). A preset def with a brand-new name that
        // clashes with a new item from an earlier preset is an accidental collision, which we
        // dedup by renaming the later one (and fixing its in-preset token_loot references).
        let vanilla_names: HashSet<String> =
            vanilla.loot_defs.iter().map(|d| d.name.clone()).collect();
        let mut used_names: HashSet<String> =
            merged.loot_defs.iter().map(|d| d.name.clone()).collect();

        for folder_name in self.preset_manager.enabled_presets() {
            if folder_name == "Vanilla (Base)" {
                continue;
            }
            if let Some(preset_data) = self.preset_manager.get_preset_loot(folder_name) {
                let mut preset = LootCatalog::load_from_bytes(&preset_data)
                    .map_err(|e| format!("Failed to parse preset '{}': {}", folder_name, e))?;

                // Pass 1: compute the final name for each def by index. Vanilla-name defs are
                // modifications (keep name). New-name defs that clash with an already-used name get
                // a unique name. Indexing by position (not by name) means even a preset that
                // contains two identically named new items keeps both instead of collapsing them.
                let mut final_names: Vec<String> = Vec::with_capacity(preset.loot_defs.len());
                let mut rename: HashMap<String, String> = HashMap::new();
                for def in &preset.loot_defs {
                    if vanilla_names.contains(&def.name) {
                        final_names.push(def.name.clone());
                        continue; // modification of a vanilla item, keep name
                    }
                    if used_names.contains(&def.name) {
                        let unique = Self::unique_loot_name(&used_names, &def.name);
                        used_names.insert(unique.clone());
                        rename.insert(def.name.clone(), unique.clone());
                        final_names.push(unique);
                    } else {
                        used_names.insert(def.name.clone());
                        final_names.push(def.name.clone());
                    }
                }

                // Pass 2: apply the final names and remap in-preset token_loot references.
                // token_loot remapping is best-effort when names were duplicated in one preset.
                for (i, def) in preset.loot_defs.iter_mut().enumerate() {
                    def.name = final_names[i].clone();
                    if let Some(new_ref) = rename.get(&def.token_loot) {
                        def.token_loot = new_ref.clone();
                    }
                }

                // Pass 3: merge into the result (replace vanilla matches, append the rest).
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

        // Disabled items are NOT removed here: this merge is the full catalog used for the working
        // view (so disabled entries remain re-enableable). The exported loot.zls filters them out.

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

    /// Merge magic overrides from all enabled presets (later ones override earlier ones).
    fn merged_magic_overrides(
        &self,
    ) -> Result<HashMap<String, HashMap<i32, MagicSlotOverrides>>, String> {
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
        Ok(merged)
    }

    /// Serialize one magic slot value (selected by `select`) into the weapon -> {x,y,b} format the
    /// loader reads. Only non-vanilla (!= 1.0) values are written to keep the files small.
    fn magic_slot_json(
        merged: &HashMap<String, HashMap<i32, MagicSlotOverrides>>,
        select: impl Fn(&MagicSlotOverrides) -> f32,
    ) -> Result<String, String> {
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
                let v = select(over);
                if (v - 1.0).abs() > 0.0001 {
                    weapon_entry.insert(label.to_string(), serde_json::json!(v));
                }
            }
            if !weapon_entry.is_empty() {
                output.insert(weapon.clone(), serde_json::Value::Object(weapon_entry));
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
        // Disabled monsters are kept in this full merge (for the working view); the exported
        // monsters.zms filters them out.

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

    /// Full path of the preset folder currently being edited (override or GUID based).
    pub fn preset_folder_path(&self) -> PathBuf {
        self.preset_manager
            .presets_dir()
            .join(&self.edit_folder_name)
    }

    pub fn save_preset(&mut self, folder_name: &str, meta: PresetMeta) -> Result<(), String> {
        let loot_delta = self.build_delta_catalog()?;
        self.preset_manager
            .save_preset_loot(folder_name, &loot_delta)?;

        // Disabled items (kept in the catalog but excluded from export). Sorted for stable output.
        let mut loot_disabled: Vec<String> = self.loot_disabled.iter().cloned().collect();
        loot_disabled.sort();
        let loot_dis_bytes = serde_json::to_vec(&loot_disabled)
            .map_err(|e| format!("Failed to serialize loot disabled: {}", e))?;
        self.preset_manager
            .save_preset_file(folder_name, "loot_disabled.json", &loot_dis_bytes)?;

        if let Some(_) = &self.working_monster_catalog {
            let monster_delta = self.build_delta_monster_catalog()?;
            self.preset_manager
                .save_preset_file(folder_name, "monsters.zms", &monster_delta)?;
            let mut monster_disabled: Vec<String> =
                self.monster_disabled.iter().cloned().collect();
            monster_disabled.sort();
            let monster_dis_bytes = serde_json::to_vec(&monster_disabled)
                .map_err(|e| format!("Failed to serialize monster disabled: {}", e))?;
            self.preset_manager.save_preset_file(
                folder_name,
                "monster_disabled.json",
                &monster_dis_bytes,
            )?;
        }
        let magic_bytes = serde_json::to_vec(&self.magic_slot_overrides)
            .map_err(|e| format!("Failed to serialize magic overrides: {}", e))?;
        self.preset_manager
            .save_preset_file(folder_name, "magic_overrides.json", &magic_bytes)?;

        let shop_bytes = serde_json::to_vec(&self.shop_additions)
            .map_err(|e| format!("Failed to serialize shop additions: {}", e))?;
        self.preset_manager
            .save_preset_file(folder_name, "shop_additions.json", &shop_bytes)?;

        let craft_bytes = serde_json::to_vec(&self.craft_additions)
            .map_err(|e| format!("Failed to serialize craft additions: {}", e))?;
        self.preset_manager
            .save_preset_file(folder_name, "craft_additions.json", &craft_bytes)?;

        // Snapshot the working assets (textures, master.zcm, char defs, icons) into the preset.
        self.save_assets_to_preset(folder_name)?;

        self.preset_manager.save_preset_meta(folder_name, &meta)?;
        self.preset_manager.refresh();
        Ok(())
    }

    pub(crate) fn apply_enabled_presets(&mut self) {
        // Loot: working view keeps the full merge (disabled items stay, re-enableable); the
        // exported loot.zls has disabled items removed so the game does not load them.
        match self.merge_enabled_presets() {
            Ok(merged_loot) => match LootCatalog::load_from_bytes(&merged_loot) {
                Ok(full) => {
                    let disabled = self.collect_loot_disabled();
                    self.loot_disabled = disabled.clone();
                    if let Some(gp) = &self.game_path {
                        let dest = gp.join("BepInEx/config/amione.SaS2Resalter/loot.zls");
                        match loot_bytes_excluding(&full, &disabled) {
                            Ok(bytes) => {
                                if let Err(e) = std::fs::create_dir_all(dest.parent().unwrap()) {
                                    self.error_message =
                                        Some(format!("Failed to create directory: {}", e));
                                } else if let Err(e) = std::fs::write(&dest, &bytes) {
                                    self.error_message =
                                        Some(format!("Failed to write loot.zls: {}", e));
                                } else {
                                    self.error_message = None;
                                }
                            }
                            Err(e) => self.error_message = Some(e),
                        }
                    }
                    self.working_catalog = Some(full);
                }
                Err(_) => self.error_message = Some("Failed to parse merged loot.zls".to_string()),
            },
            Err(e) => self.error_message = Some(e),
        }

        // Monsters: same model as loot.
        match self.merge_enabled_monster_presets() {
            Ok(merged_monsters) => match MonsterCatalog::load_from_bytes(&merged_monsters) {
                Ok(full) => {
                    let disabled = self.collect_monster_disabled();
                    self.monster_disabled = disabled.clone();
                    if let Some(gp) = &self.game_path {
                        let dest = gp.join("BepInEx/config/amione.SaS2Resalter/monsters.zms");
                        match monster_bytes_excluding(&full, &disabled) {
                            Ok(bytes) => {
                                if let Err(e) = std::fs::create_dir_all(dest.parent().unwrap()) {
                                    self.error_message =
                                        Some(format!("Failed to create directory: {}", e));
                                } else if let Err(e) = std::fs::write(&dest, &bytes) {
                                    self.error_message =
                                        Some(format!("Failed to write monsters.zms: {}", e));
                                } else {
                                    self.error_message = None;
                                }
                            }
                            Err(e) => self.error_message = Some(e),
                        }
                    }
                    self.working_monster_catalog = Some(full);
                }
                Err(_) => {
                    self.error_message = Some("Failed to parse merged monsters.zms".to_string())
                }
            },
            Err(e) => self.error_message = Some(e),
        }

        // Write magic_damage.json, magic_cost.json and magic_cooldown.json (per-weapon, per-slot).
        match self.merged_magic_overrides() {
            Ok(merged) => {
                if let Some(gp) = self.game_path.clone() {
                    let config_dir = gp.join("BepInEx/config/amione.SaS2Resalter");
                    if let Err(e) = std::fs::create_dir_all(&config_dir) {
                        self.error_message = Some(format!("Failed to create config dir: {}", e));
                    } else {
                        for (file, select) in [
                            ("magic_damage.json", &(|o: &MagicSlotOverrides| o.damage) as &dyn Fn(&MagicSlotOverrides) -> f32),
                            ("magic_cost.json", &(|o: &MagicSlotOverrides| o.cost)),
                            ("magic_cooldown.json", &(|o: &MagicSlotOverrides| o.cooldown)),
                        ] {
                            match Self::magic_slot_json(&merged, select) {
                                Ok(json) => {
                                    if let Err(e) = std::fs::write(config_dir.join(file), json) {
                                        self.error_message =
                                            Some(format!("Failed to write {}: {}", file, e));
                                    }
                                }
                                Err(e) => self.error_message = Some(e),
                            }
                        }
                    }
                }
            }
            Err(e) => self.error_message = Some(e),
        }

        // Write shop_additions.txt and craft_additions.txt (one "item" or "flag:item" per line).
        let shop_text = self.merged_additions_text("shop_additions.json");
        let craft_text = self.merged_additions_text("craft_additions.json");
        if let Some(gp) = &self.game_path {
            let config_dir = gp.join("BepInEx/config/amione.SaS2Resalter");
            if let Err(e) = std::fs::create_dir_all(&config_dir) {
                self.error_message = Some(format!("Failed to create config dir: {}", e));
            } else {
                if let Err(e) = std::fs::write(config_dir.join("shop_additions.txt"), shop_text) {
                    self.error_message =
                        Some(format!("Failed to write shop_additions.txt: {}", e));
                }
                if let Err(e) = std::fs::write(config_dir.join("craft_additions.txt"), craft_text) {
                    self.error_message =
                        Some(format!("Failed to write craft_additions.txt: {}", e));
                }
            }
        }

        // Merge binary assets (textures, master.zcm, char defs, custom icons) into the config.
        self.merge_assets_to_config();
    }

    /// Merge per-item additions (shop or craft) from all enabled presets into the loader's line
    /// format. Each entry is "item" or "flag:item". Later presets override an item's flag.
    fn merged_additions_text(&self, sidecar: &str) -> String {
        let mut merged: HashMap<String, String> = HashMap::new();
        for folder_name in self.preset_manager.enabled_presets() {
            if folder_name == "Vanilla (Base)" {
                continue;
            }
            if let Some(data) = self.preset_manager.get_preset_file(folder_name, sidecar) {
                if let Ok(map) = serde_json::from_slice::<HashMap<String, String>>(&data) {
                    for (item, flag) in map {
                        merged.insert(item, flag);
                    }
                }
            }
        }

        let mut lines: Vec<String> = merged
            .into_iter()
            .map(|(item, flag)| {
                let flag = flag.trim();
                if flag.is_empty() {
                    item
                } else {
                    format!("{}:{}", flag, item)
                }
            })
            .collect();
        lines.sort();
        lines.join("\n")
    }

    /// Load a preset folder into the working catalog by merging it on top of vanilla.
    /// This keeps all vanilla items and only changes the ones the preset modifies.
    pub fn load_preset(&mut self, folder_name: &str) {
        // Sync the working-assets folder to this preset's assets (resets the asset editors).
        self.load_assets_from_preset(folder_name);

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

                    // Load this preset's disabled set (items stay in the catalog, just marked).
                    if let Some(dis) = self
                        .preset_manager
                        .get_preset_file(folder_name, "loot_disabled.json")
                    {
                        let names: Vec<String> = serde_json::from_slice(&dis).unwrap_or_default();
                        self.loot_disabled = names.into_iter().collect();
                    } else {
                        self.loot_disabled.clear();
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

                    // Load shop additions (if present), otherwise start empty.
                    if let Some(data) = self
                        .preset_manager
                        .get_preset_file(folder_name, "shop_additions.json")
                    {
                        self.shop_additions = serde_json::from_slice(&data).unwrap_or_default();
                    } else {
                        self.shop_additions.clear();
                    }

                    // Load craft additions (if present), otherwise start empty.
                    if let Some(data) = self
                        .preset_manager
                        .get_preset_file(folder_name, "craft_additions.json")
                    {
                        self.craft_additions = serde_json::from_slice(&data).unwrap_or_default();
                    } else {
                        self.craft_additions.clear();
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

                    // Load this preset's disabled monster set (kept in catalog, just marked).
                    if let Some(dis) = self
                        .preset_manager
                        .get_preset_file(folder_name, "monster_disabled.json")
                    {
                        let names: Vec<String> = serde_json::from_slice(&dis).unwrap_or_default();
                        self.monster_disabled = names.into_iter().collect();
                    } else {
                        self.monster_disabled.clear();
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
            self.folder_override_enabled = p.meta.folder_override;
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

                    ui.separator();
                    ui.heading("Items");
                    if ui
                        .checkbox(
                            &mut self.config.auto_type_fields,
                            "Auto-fill fields by item type (new items / type changes)",
                        )
                        .on_hover_text(
                            "New items and items whose type changes get that type's field set, \
                             copied from a vanilla item of the type (existing field values kept).",
                        )
                        .changed()
                    {
                        self.config_save_timer = 0.1;
                    }

                    ui.separator();
                    ui.heading("Textures");
                    ui.label("External image editor (blank = OS default):");
                    ui.horizontal(|ui| {
                        if ui
                            .text_edit_singleline(&mut self.config.external_image_editor)
                            .changed()
                        {
                            self.config_save_timer = 0.1;
                        }
                        if ui.button("Browse").clicked() {
                            if let Some(path) = FileDialog::new().pick_file() {
                                self.config.external_image_editor =
                                    path.to_string_lossy().into_owned();
                                self.config_save_timer = 0.1;
                            }
                        }
                        if ui.button("Clear").clicked() {
                            self.config.external_image_editor.clear();
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
                ui.selectable_value(&mut self.active_tab, Tab::Textures, "Textures");
                ui.selectable_value(&mut self.active_tab, Tab::Animations, "Animations");
                ui.selectable_value(&mut self.active_tab, Tab::Images, "Images");

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
                Tab::Textures => textures::show(self, ui),
                Tab::Animations => animations::show(self, ui),
                Tab::Images => images::show(self, ui),
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
