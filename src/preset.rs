use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use zip::write::FileOptions;
use zip::ZipWriter;

/// Metadata stored inside each preset's folder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetMeta {
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
}

/// A preset located in the presets folder.
pub struct Preset {
    pub folder_name: String, // directory name inside presets/
    pub meta: PresetMeta,
    pub loot_data: Vec<u8>, // contents of loot.zls
}

pub struct PresetManager {
    /// Directory where all presets are stored.
    presets_dir: PathBuf,
    /// Mod config file (BepInEx/config/SaS2Salter.ini)
    cfg_path: Option<PathBuf>,
    /// Ordered list of enabled preset folder names (as read from config).
    enabled_presets: Vec<String>,
    /// Cache of all installed presets, keyed by folder name.
    installed_presets: Vec<Preset>,
    vanilla_data: Option<Vec<u8>>,
}

impl PresetManager {
    /// Create a new manager. Call `set_game_path` afterwards to set the INI location.
    pub fn new() -> Self {
        let presets_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sas2-resalinated")
            .join("presets");
        fs::create_dir_all(&presets_dir).ok();

        Self {
            presets_dir,
            cfg_path: None,
            enabled_presets: Vec::new(),
            installed_presets: Vec::new(),
            vanilla_data: None,
        }
    }

    pub fn set_vanilla_data(&mut self, data: Vec<u8>) {
        self.vanilla_data = Some(data);
        self.ensure_vanilla_preset_exists();
    }

    /// Set the game folder and read the current enabled presets from the mod config.
    pub fn set_game_path(&mut self, game_path: &Path) {
        self.cfg_path = Some(game_path.join("BepInEx/config/amione.SaS2Resalter.cfg"));
        self.refresh();
    }

    /// Reload installed presets and enabled list.
    pub fn refresh(&mut self) {
        self.load_installed_presets();
        self.load_enabled_presets();
        self.ensure_vanilla_preset_exists();
    }

    fn ensure_vanilla_preset_exists(&mut self) {
        let vanilla_name = "Vanilla (Base)";
        let dir = self.presets_dir.join(vanilla_name);
        // Write or overwrite meta and loot if we have vanilla data
        if let Some(ref data) = self.vanilla_data {
            let meta = PresetMeta {
                name: "Vanilla".to_string(),
                version: "1.0.0".to_string(),
                author: "Ska Studios".to_string(),
                description: "Original game items".to_string(),
            };
            // Always overwrite to ensure latest vanilla data
            let _ = fs::create_dir_all(&dir);
            let meta_json = serde_json::to_string_pretty(&meta).unwrap_or_default();
            let _ = fs::write(dir.join("preset.json"), meta_json);
            let _ = fs::write(dir.join("loot.zls"), data);
        }
        // Reload installed presets to pick up the new/updated vanilla
        self.load_installed_presets();
        // Ensure vanilla is always first in enabled list if not already present
        if !self.enabled_presets.contains(&vanilla_name.to_string()) {
            self.enabled_presets.insert(0, vanilla_name.to_string());
            self.save_enabled_presets();
        }
    }

    fn load_installed_presets(&mut self) {
        self.installed_presets.clear();
        if let Ok(entries) = fs::read_dir(&self.presets_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let folder_name = path.file_name().unwrap().to_string_lossy().to_string();
                    if let Some(preset) = self.read_preset(&folder_name) {
                        self.installed_presets.push(preset);
                    }
                }
            }
        }
    }

    fn read_preset(&self, folder_name: &str) -> Option<Preset> {
        let dir = self.presets_dir.join(folder_name);
        let meta_path = dir.join("preset.json");
        let loot_path = dir.join("loot.zls");
        if !meta_path.exists() || !loot_path.exists() {
            return None;
        }
        let meta_str = fs::read_to_string(&meta_path).ok()?;
        let meta: PresetMeta = serde_json::from_str(&meta_str).ok()?;
        let loot_data = fs::read(&loot_path).ok()?;
        Some(Preset {
            folder_name: folder_name.to_string(),
            meta,
            loot_data,
        })
    }

    fn load_enabled_presets(&mut self) {
        self.enabled_presets.clear();
        let Some(ini_path) = &self.cfg_path else {
            return;
        };
        let Ok(file) = File::open(ini_path) else {
            return;
        };
        let reader = BufReader::new(file);
        for line in reader.lines().flatten() {
            let line = line.trim();
            if line.starts_with("enabledPresets") {
                // Example: enabledPresets = ThisPreset, ThatPreset, WoahPreset
                if let Some(val) = line.split('=').nth(1) {
                    self.enabled_presets = val
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                break;
            }
        }
    }

    /// Write the enabled presets back to the INI file.
    pub fn save_enabled_presets(&self) {
        let Some(ini_path) = &self.cfg_path else {
            return;
        };
        let content = match fs::read_to_string(ini_path) {
            Ok(c) => c,
            Err(_) => {
                // If file doesn't exist, create it with a [General] section
                let mut default = String::from("[General]\n");
                default.push_str(&format!(
                    "enabledPresets = {}\n",
                    self.enabled_presets.join(", ")
                ));
                let _ = fs::write(ini_path, &default);
                return;
            }
        };

        let mut new_lines = Vec::new();
        let mut replaced = false;
        for line in content.lines() {
            if line.trim_start().starts_with("enabledPresets") {
                new_lines.push(format!(
                    "enabledPresets = {}",
                    self.enabled_presets.join(", ")
                ));
                replaced = true;
            } else {
                new_lines.push(line.to_string());
            }
        }
        if !replaced {
            new_lines.push(format!(
                "enabledPresets = {}",
                self.enabled_presets.join(", ")
            ));
        }
        let _ = fs::write(ini_path, new_lines.join("\n"));
    }

    pub fn installed_presets(&self) -> &[Preset] {
        &self.installed_presets
    }

    pub fn enabled_presets(&self) -> &[String] {
        &self.enabled_presets
    }

    /// Move a preset from available to enabled.
    pub fn enable_preset(&mut self, folder_name: &str) {
        if !self.enabled_presets.contains(&folder_name.to_string()) {
            self.enabled_presets.push(folder_name.to_string());
            self.save_enabled_presets();
        }
    }

    /// Remove a preset from the enabled list.
    pub fn disable_preset(&mut self, folder_name: &str) -> Result<(), String> {
        if folder_name == "Vanilla (Base)" {
            return Err("Cannot disable the Vanilla preset".to_string());
        }
        self.enabled_presets.retain(|x| x != folder_name);
        self.save_enabled_presets();
        Ok(())
    }

    /// Reorder enabled presets: move the preset at `from_index` to `to_index`.
    pub fn move_preset(&mut self, from_index: usize, to_index: usize) {
        if from_index < self.enabled_presets.len() && to_index < self.enabled_presets.len() {
            let item = self.enabled_presets.remove(from_index);
            self.enabled_presets.insert(to_index, item);
            self.save_enabled_presets();
        }
    }

    /// Import a preset from a zip file. Expects a folder at the root of the zip.
    pub fn import_preset(&mut self, zip_path: &Path) -> Result<(), String> {
        let file = File::open(zip_path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

        // Find the common prefix (top-level folder)
        let mut folder_name = String::new();
        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(|e| e.to_string())?;
            if entry.is_dir() {
                folder_name = entry.name().unwrap().trim_end_matches('/').to_string();
                break;
            }
        }
        if folder_name.is_empty() {
            return Err("No top-level folder found in zip".to_string());
        }

        let dest_dir = self.presets_dir.join(&folder_name);
        if dest_dir.exists() {
            return Err(format!("Preset '{}' already exists", folder_name));
        }
        fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;

        // Extract all files inside that folder
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let entry_path = entry.name().unwrap().to_string();
            // Strip the top-level folder name
            let relative = entry_path
                .strip_prefix(&folder_name)
                .unwrap_or(&entry_path)
                .trim_start_matches('/')
                .trim_start_matches('\\');
            if relative.is_empty() {
                continue;
            }
            let out_path = dest_dir.join(relative);
            if entry.is_dir() {
                fs::create_dir_all(&out_path).map_err(|e| e.to_string())?;
            } else {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                let mut outfile = File::create(&out_path).map_err(|e| e.to_string())?;
                std::io::copy(&mut entry, &mut outfile).map_err(|e| e.to_string())?;
            }
        }

        self.refresh();
        Ok(())
    }

    /// Export a preset to a ZIP file.
    pub fn export_preset(&self, folder_name: &str, dest_file: &Path) -> Result<(), String> {
        let src_dir = self.presets_dir.join(folder_name);
        if !src_dir.exists() {
            return Err(format!("Preset folder '{}' not found", folder_name));
        }

        let file = File::create(dest_file).map_err(|e| e.to_string())?;
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        fn add_dir_to_zip(
            zip: &mut ZipWriter<File>,
            base: &Path,
            prefix: &Path,
            options: FileOptions<()>,
        ) -> Result<(), String> {
            for entry in fs::read_dir(base).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let path = entry.path();
                let relative = prefix.join(path.file_name().unwrap());
                if path.is_dir() {
                    zip.add_directory(relative.to_string_lossy(), options)
                        .map_err(|e| e.to_string())?;
                    add_dir_to_zip(zip, &path, &relative, options)?;
                } else {
                    zip.start_file(relative.to_string_lossy(), options)
                        .map_err(|e| e.to_string())?;
                    let data = fs::read(&path).map_err(|e| e.to_string())?;
                    zip.write_all(&data).map_err(|e| e.to_string())?;
                }
            }
            Ok(())
        }

        // Add the folder itself as the root entry
        zip.add_directory(folder_name, options)
            .map_err(|e| e.to_string())?;
        add_dir_to_zip(&mut zip, &src_dir, &PathBuf::from(folder_name), options)?;
        zip.finish().map_err(|e| e.to_string())?;

        Ok(())
    }

    /// Read any file from a preset folder.
    pub fn get_preset_file(&self, folder_name: &str, filename: &str) -> Option<Vec<u8>> {
        let path = self.presets_dir.join(folder_name).join(filename);
        fs::read(&path).ok()
    }

    /// Write any file into a preset folder.
    pub fn save_preset_file(
        &self,
        folder_name: &str,
        filename: &str,
        data: &[u8],
    ) -> Result<(), String> {
        let dir = self.presets_dir.join(folder_name);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(filename);
        fs::write(&path, data).map_err(|e| e.to_string())
    }

    /// Delete a preset entirely.
    pub fn delete_preset(&mut self, folder_name: &str) -> Result<(), String> {
        if folder_name == "Vanilla (Base)" {
            return Err("Cannot delete the Vanilla preset".to_string());
        }
        let dir = self.presets_dir.join(folder_name);
        if dir.exists() {
            fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
            self.refresh();
        }
        Ok(())
    }

    /// Read the loot data for an installed preset.
    pub fn get_preset_loot(&self, folder_name: &str) -> Option<Vec<u8>> {
        let loot_path = self.presets_dir.join(folder_name).join("loot.zls");
        fs::read(&loot_path).ok()
    }

    // Returns the presets directory
    pub fn presets_dir(&self) -> &Path {
        &self.presets_dir
    }

    /// Save metadata for an installed preset.
    pub fn save_preset_meta(&self, folder_name: &str, meta: &PresetMeta) -> Result<(), String> {
        let dir = self.presets_dir.join(folder_name);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let meta_path = dir.join("preset.json");
        let json = serde_json::to_string_pretty(meta).map_err(|e| e.to_string())?;
        fs::write(&meta_path, json).map_err(|e| e.to_string())
    }

    /// Save loot data for an installed preset.
    pub fn save_preset_loot(&self, folder_name: &str, data: &[u8]) -> Result<(), String> {
        let dir = self.presets_dir.join(folder_name);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let loot_path = dir.join("loot.zls");
        fs::write(&loot_path, data).map_err(|e| e.to_string())
    }

    /// Create a new preset with the given folder name and metadata.
    pub fn create_preset(
        &mut self,
        folder_name: &str,
        meta: PresetMeta,
        loot_data: &[u8],
    ) -> Result<(), String> {
        let dir = self.presets_dir.join(folder_name);
        if dir.exists() {
            return Err(format!("Preset '{}' already exists", folder_name));
        }
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        self.save_preset_meta(folder_name, &meta)?;
        self.save_preset_loot(folder_name, loot_data)?;
        self.refresh();
        Ok(())
    }
}
