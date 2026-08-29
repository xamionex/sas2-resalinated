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
    /// Editor version that last saved this preset. None for presets saved
    /// before this field existed.
    #[serde(default)]
    pub editor_version: Option<String>,
    /// True when the user manually chose the folder name (override). Such
    /// presets are never auto-renamed by the GUID migration.
    #[serde(default)]
    pub folder_override: bool,
}

/// Make a folder name valid on both Windows and Linux: replace forbidden
/// characters, strip trailing dots/spaces, prefix Windows reserved device
/// names, and cap the length. Never returns an empty string.
pub fn sanitize_folder_name(name: &str) -> String {
    let mut out: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '\0' => '_',
            c if (c as u32) < 0x20 => '_',
            c => c,
        })
        .collect();
    while out.ends_with('.') || out.ends_with(' ') {
        out.pop();
    }
    if out.is_empty() {
        return "preset".to_string();
    }
    let stem = out.split('.').next().unwrap_or("").to_uppercase();
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    if RESERVED.contains(&stem.as_str()) {
        out.insert(0, '_');
    }
    if out.len() > 200 {
        let mut end = 200;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
    }
    out
}

/// Deterministic 64-bit FNV-1a hash. Stable across platforms and Rust versions
/// (unlike `DefaultHasher`), so folder names never change between builds.
fn fnv1a64(data: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Folder name for a preset, derived from its author, name and version.
/// The readable prefix keeps the Manager list recognizable; the 64-bit hash
/// of the triple is the GUID that makes the folder unique per preset identity.
/// The same author/name/version always maps to the same folder (re-saving
/// updates it), while different packs never collide.
pub fn guid_folder_name(meta: &PresetMeta) -> String {
    let slug = |s: &str| {
        let mut out = sanitize_folder_name(s);
        if out.len() > 40 {
            let mut end = 40;
            while !out.is_char_boundary(end) {
                end -= 1;
            }
            out.truncate(end);
        }
        out
    };
    let identity = format!("{}\0{}\0{}", meta.author, meta.name, meta.version);
    format!(
        "{}.{}.{}.{:016x}",
        slug(&meta.author),
        slug(&meta.name),
        slug(&meta.version),
        fnv1a64(identity.as_bytes())
    )
}

/// A preset located in the presets folder.
pub struct Preset {
    pub folder_name: String, // directory name inside presets/
    pub meta: PresetMeta,
    pub loot_data: Vec<u8>, // contents of loot.zls
}

impl Preset {
    /// Human-friendly label for the Manager lists: "Name by Author (version)".
    /// The vanilla preset keeps its short folder name.
    pub fn display_name(&self) -> String {
        if self.folder_name == "Vanilla (Base)" {
            return "Vanilla (Base)".to_string();
        }
        format!(
            "{} by {} ({})",
            self.meta.name, self.meta.author, self.meta.version
        )
    }
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
        self.migrate_legacy_folders();
        self.ensure_vanilla_preset_exists();
    }

    /// Rename presets saved before the GUID folder scheme to their GUID folder
    /// name. Runs automatically so users never notice the change. Presets with
    /// a manually chosen folder (override) and the vanilla preset are kept as
    /// they are. The enabled list is rewritten with the new names.
    fn migrate_legacy_folders(&mut self) {
        let mut renamed: Vec<(String, String)> = Vec::new();
        for preset in &self.installed_presets {
            if preset.folder_name == "Vanilla (Base)" || preset.meta.folder_override {
                continue;
            }
            let guid = guid_folder_name(&preset.meta);
            if guid != preset.folder_name {
                renamed.push((preset.folder_name.clone(), guid));
            }
        }
        if renamed.is_empty() {
            return;
        }
        for (old, new) in &renamed {
            let old_dir = self.presets_dir.join(old);
            let new_dir = self.presets_dir.join(new);
            if new_dir.exists() {
                // A preset with the same identity already exists at the GUID
                // name; the legacy copy is redundant, so drop it.
                let _ = fs::remove_dir_all(&old_dir);
            } else if fs::rename(&old_dir, &new_dir).is_err() {
                continue;
            }
            self.enabled_presets.retain(|e| e != old);
            if !self.enabled_presets.contains(new) {
                self.enabled_presets.push(new.clone());
            }
        }
        self.save_enabled_presets();
        self.load_installed_presets();
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
                editor_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                folder_override: false,
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
    /// Works with zips that have explicit directory entries and with zips that
    /// only have file entries (e.g. made by Windows Explorer). If a preset with
    /// the same folder name already exists it is replaced, so older versions of
    /// a pack can be re-imported to update it.
    pub fn import_preset(&mut self, zip_path: &Path) -> Result<(), String> {
        let file = File::open(zip_path).map_err(|e| e.to_string())?;
        let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

        // Derive the top-level folder from the first entry's path, so zips
        // without explicit directory entries (Windows Explorer style) import too.
        let mut folder_name = String::new();
        for i in 0..archive.len() {
            let entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let name = entry.name().map_err(|e| e.to_string())?.to_string();
            let name = name.trim_end_matches('/').trim_end_matches('\\');
            let first = name.split(['/', '\\']).next().unwrap_or("");
            if !first.is_empty() {
                folder_name = first.to_string();
                break;
            }
        }
        if folder_name.is_empty() {
            return Err("No top-level folder found in zip".to_string());
        }

        let dest_dir = self.presets_dir.join(&folder_name);
        if dest_dir.exists() {
            fs::remove_dir_all(&dest_dir).map_err(|e| e.to_string())?;
        }
        fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;

        // Extract all files inside that folder
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
            let entry_path = entry.name().map_err(|e| e.to_string())?.to_string();
            // Normalize Windows separators, then strip the top-level folder name.
            let normalized = entry_path.replace('\\', "/");
            let relative = normalized
                .strip_prefix(&folder_name)
                .unwrap_or(&normalized)
                .trim_start_matches('/');
            if relative.is_empty() {
                continue;
            }
            // Reject entries that would escape the preset folder.
            if relative
                .split('/')
                .any(|part| part == ".." || part.contains('\0'))
            {
                return Err(format!("Zip entry '{}' has an unsafe path", entry_path));
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

        // Stamp the current editor version so presets exported by an older
        // editor are treated as current format from here on.
        if let Some(preset) = self.read_preset(&folder_name) {
            let _ = self.save_preset_meta(&folder_name, &preset.meta);

            // Old editor zips use a user-defined folder; move the preset to its
            // GUID name, replacing any existing preset with the same identity so
            // re-importing an older version updates it.
            if !preset.meta.folder_override {
                let guid = guid_folder_name(&preset.meta);
                if guid != folder_name {
                    let old_dir = self.presets_dir.join(&folder_name);
                    let guid_dir = self.presets_dir.join(&guid);
                    if guid_dir.exists() {
                        fs::remove_dir_all(&guid_dir).map_err(|e| e.to_string())?;
                    }
                    fs::rename(&old_dir, &guid_dir).map_err(|e| e.to_string())?;
                    self.enabled_presets.retain(|e| e != &folder_name);
                    if !self.enabled_presets.contains(&guid) {
                        self.enabled_presets.push(guid);
                    }
                    self.save_enabled_presets();
                }
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

    /// Save metadata for an installed preset. Always stamps the current editor
    /// version so the save format can be checked in the future.
    pub fn save_preset_meta(&self, folder_name: &str, meta: &PresetMeta) -> Result<(), String> {
        let mut stamped = meta.clone();
        stamped.editor_version = Some(env!("CARGO_PKG_VERSION").to_string());
        let dir = self.presets_dir.join(folder_name);
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let meta_path = dir.join("preset.json");
        let json = serde_json::to_string_pretty(&stamped).map_err(|e| e.to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(author: &str, name: &str, version: &str) -> PresetMeta {
        PresetMeta {
            name: name.to_string(),
            version: version.to_string(),
            author: author.to_string(),
            description: String::new(),
            editor_version: None,
            folder_override: false,
        }
    }

    #[test]
    fn guid_is_deterministic() {
        let m = meta("Ska Studios", "Balance", "1.0.0");
        assert_eq!(guid_folder_name(&m), guid_folder_name(&m));
    }

    #[test]
    fn guid_differs_between_packs() {
        let a = guid_folder_name(&meta("Ska Studios", "Balance", "1.0.0"));
        let b = guid_folder_name(&meta("Other Author", "Balance", "1.0.0"));
        let c = guid_folder_name(&meta("Ska Studios", "Balance", "2.0.0"));
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn guid_is_safe_on_windows_and_linux() {
        let m = meta("A:B/C*D?E\"F<G>H|I", "Name.", "1.0.0");
        let name = guid_folder_name(&m);
        assert!(!name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']));
        assert!(!name.ends_with('.') && !name.ends_with(' '));
        assert!(name.len() <= 200);
    }

    #[test]
    fn sanitize_handles_reserved_names() {
        assert_eq!(sanitize_folder_name("CON"), "_CON");
        assert_eq!(sanitize_folder_name("con.txt"), "_con.txt");
        assert_eq!(sanitize_folder_name(".."), "preset");
        assert_eq!(sanitize_folder_name("  "), "preset");
    }

    #[test]
    fn old_meta_parses_with_defaults() {
        // A preset.json written before editor_version/folder_override existed.
        let json = r#"{"name":"Balance","version":"1.0.0","author":"Ska Studios","description":"d"}"#;
        let meta: PresetMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.editor_version, None);
        assert!(!meta.folder_override);
    }

    #[test]
    fn save_meta_stamps_editor_version() {
        let dir = std::env::temp_dir().join(format!("sas2-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let manager = PresetManager {
            presets_dir: dir.clone(),
            cfg_path: None,
            enabled_presets: Vec::new(),
            installed_presets: Vec::new(),
            vanilla_data: None,
        };
        let m = meta("Ska Studios", "Balance", "1.0.0");
        manager.save_preset_meta("test", &m).unwrap();
        let saved: PresetMeta =
            serde_json::from_str(&fs::read_to_string(dir.join("test/preset.json")).unwrap())
                .unwrap();
        assert_eq!(
            saved.editor_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn import_old_zip_without_dir_entries() {
        let dir = std::env::temp_dir().join(format!("sas2-import-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("presets")).unwrap();
        fs::write(dir.join("test.cfg"), "[General]\nenabledPresets = \n").unwrap();

        let zip_path = dir.join("old.zip");
        let file = File::create(&zip_path).unwrap();
        let mut zip = ZipWriter::new(file);
        let options: FileOptions<'_, '_, ()> =
            FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        // Windows Explorer style: no explicit directory entries, and the old
        // app wrote preset.json without editor_version/folder_override.
        zip.start_file("My Old Preset/preset.json", options).unwrap();
        zip.write_all(
            br#"{"name":"Balance","version":"1.0.0","author":"Ska Studios","description":"d"}"#,
        )
        .unwrap();
        zip.start_file("My Old Preset/loot.zls", options).unwrap();
        zip.write_all(b"lootdata").unwrap();
        zip.finish().unwrap();

        let mut manager = PresetManager {
            presets_dir: dir.join("presets"),
            cfg_path: Some(dir.join("test.cfg")),
            enabled_presets: Vec::new(),
            installed_presets: Vec::new(),
            vanilla_data: None,
        };
        manager.import_preset(&zip_path).unwrap();

        // Old user-defined folders are migrated to the GUID name on import.
        let guid = guid_folder_name(&meta("Ska Studios", "Balance", "1.0.0"));
        let names: Vec<String> = manager
            .installed_presets
            .iter()
            .map(|p| p.folder_name.clone())
            .collect();
        assert_eq!(names, vec![guid.clone()]);
        assert!(dir.join("presets").join(&guid).join("loot.zls").exists());

        // Imported presets get the current editor version stamped.
        let saved: PresetMeta = serde_json::from_str(
            &fs::read_to_string(dir.join("presets").join(&guid).join("preset.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            saved.editor_version.as_deref(),
            Some(env!("CARGO_PKG_VERSION"))
        );

        // Re-importing the same preset replaces it instead of erroring.
        manager.import_preset(&zip_path).unwrap();
        let names: Vec<String> = manager
            .installed_presets
            .iter()
            .map(|p| p.folder_name.clone())
            .collect();
        assert_eq!(names, vec![guid]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn migration_renames_legacy_folders() {
        let dir = std::env::temp_dir().join(format!("sas2-migrate-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Legacy preset: user-defined folder name, no editor_version.
        let legacy = meta("Ska Studios", "Balance", "1.0.0");
        let legacy_dir = dir.join("My Cool Preset");
        fs::create_dir_all(&legacy_dir).unwrap();
        fs::write(
            legacy_dir.join("preset.json"),
            serde_json::to_string(&legacy).unwrap(),
        )
        .unwrap();
        fs::write(legacy_dir.join("loot.zls"), b"loot").unwrap();

        // Override preset: must keep its folder name.
        let mut overridden = meta("Other", "Pack", "2.0.0");
        overridden.folder_override = true;
        let override_dir = dir.join("Custom Name");
        fs::create_dir_all(&override_dir).unwrap();
        fs::write(
            override_dir.join("preset.json"),
            serde_json::to_string(&overridden).unwrap(),
        )
        .unwrap();
        fs::write(override_dir.join("loot.zls"), b"loot").unwrap();

        let mut manager = PresetManager {
            presets_dir: dir.clone(),
            cfg_path: Some(dir.join("test.cfg")),
            enabled_presets: Vec::new(),
            installed_presets: Vec::new(),
            vanilla_data: None,
        };
        fs::write(
            dir.join("test.cfg"),
            "[General]\nenabledPresets = My Cool Preset, Custom Name\n",
        )
        .unwrap();
        manager.refresh();

        let names: Vec<String> = manager
            .installed_presets
            .iter()
            .map(|p| p.folder_name.clone())
            .collect();
        assert!(names.contains(&guid_folder_name(&legacy)));
        assert!(!names.contains(&"My Cool Preset".to_string()));
        assert!(names.contains(&"Custom Name".to_string()));
        assert!(manager.enabled_presets.contains(&guid_folder_name(&legacy)));
        assert!(!manager.enabled_presets.contains(&"My Cool Preset".to_string()));
        assert!(manager.enabled_presets.contains(&"Custom Name".to_string()));
        let _ = fs::remove_dir_all(&dir);
    }
}
