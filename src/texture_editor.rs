use egui::TextureHandle;
use sas2_parser::subflags::SubFlagDefCatalog;
use sas2_parser::xnb_loader::load_texture_from_path;
use sas2_parser::xtexture::{MasterTextures, XTextureRaw};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Editor state for the texture / cell metadata tab (Phase 3).
///
/// This owns its own lossless copy of `master.zcm` (cell rectangles + origins) and renders the matching sprite sheet so cells can be inspected and adjusted numerically.
/// Texture pixels are edited externally (import a PNG or open in the user's image editor), this tool does not paint pixels in-app, by design.
///
/// All edits land in the config mirror that the loader reads:
///   - cell metadata -> `BepInEx/config/amione.SaS2Resalter/Content/gfx/master.zcm`
///   - texture pixels -> `BepInEx/config/amione.SaS2Resalter/textures/<name>.png`
#[derive(Default)]
pub struct TextureEditor {
    /// Subflag defs, needed to (de)serialize master.zcm losslessly.
    pub flag_defs: Option<SubFlagDefCatalog>,
    /// Working, editable cell metadata for every texture.
    pub master: Option<MasterTextures>,
    /// Pristine vanilla copy (game master.zcm), used for reset-to-vanilla.
    pub vanilla_master: Option<MasterTextures>,
    /// Set once a load has been attempted so we do not retry every frame.
    pub load_attempted: bool,
    /// Whether cell metadata has unsaved edits.
    pub dirty: bool,

    pub search: String,
    pub selected_texture: Option<usize>,
    pub selected_cell: Option<usize>,

    /// GPU handle for the currently displayed sheet and which texture index it belongs to.
    pub sheet_handle: Option<TextureHandle>,
    pub sheet_for: Option<usize>,
    pub sheet_size: (u32, u32),

    pub zoom: f32,
    pub status: Option<String>,

    /// Edit buffer for renaming the selected texture, and the index it tracks.
    pub rename_buffer: String,
    pub rename_for: Option<usize>,
    /// For renamed vanilla textures: current name -> original vanilla name (for revert).
    renamed_from: HashMap<String, String>,

    /// mtime of the override PNG we are watching for external edits (selected texture only).
    watch_mtime: Option<SystemTime>,
    watch_path: Option<PathBuf>,
}

impl TextureEditor {
    // Overrides live in the shared working-assets folder (snapshotted into presets on Save, merged into the live config only on Apply).
    // The `game_path` argument is kept for signature stability and vanilla-source reads.
    fn textures_dir(_game_path: &Path) -> PathBuf {
        crate::assets::working_root().join(crate::assets::TEXTURES_DIR)
    }

    fn master_override_path(_game_path: &Path) -> PathBuf {
        crate::assets::working_root().join(crate::assets::MASTER_REL)
    }

    fn png_override_path(game_path: &Path, name: &str) -> PathBuf {
        Self::textures_dir(game_path).join(format!("{}.png", name))
    }

    /// Load flag defs and cell metadata once.
    /// Prefers the config-mirror master.zcm (so prior edits persist), falling back to the game's own bundle.
    pub fn ensure_loaded(&mut self, game_path: &Path) {
        if self.load_attempted {
            return;
        }
        self.load_attempted = true;
        if self.zoom == 0.0 {
            self.zoom = 1.0;
        }

        let flagdefs_path = game_path.join("Content").join("gfx").join("flagdefs.zfd");
        let flag_defs = match SubFlagDefCatalog::load_from_path(&flagdefs_path) {
            Ok(d) => d,
            Err(e) => {
                self.status = Some(format!("Failed to load flagdefs.zfd: {}", e));
                return;
            }
        };

        let override_master = Self::master_override_path(game_path);
        let game_master = game_path.join("Content").join("gfx").join("master.zcm");
        let src = if override_master.exists() {
            override_master
        } else {
            game_master.clone()
        };

        // Always keep a pristine vanilla copy for reset-to-vanilla.
        let vanilla = MasterTextures::load_from_path(&game_master, &flag_defs).ok();

        match MasterTextures::load_from_path(&src, &flag_defs) {
            Ok(m) => {
                self.master = Some(m);
                self.vanilla_master = vanilla;
                self.flag_defs = Some(flag_defs);
            }
            Err(e) => self.status = Some(format!("Failed to load master.zcm: {}", e)),
        }
    }

    /// True when the texture at `idx` exists in vanilla (i.e. it can be reset to vanilla).
    pub fn is_vanilla_texture(&self, idx: usize) -> bool {
        let name = self.texture_name(idx);
        self.vanilla_master
            .as_ref()
            .map_or(false, |v| v.get(&name).is_some())
    }

    /// Append a blank texture entry (no cells) and select it.
    /// The user imports a PNG and adds cells afterwards.
    /// Returns the new index.
    pub fn create_blank_texture(&mut self) -> Option<usize> {
        let master = self.master.as_mut()?;
        let name = unique_texture_name(master, "new_texture");
        master.entries.push((
            name.into_bytes(),
            XTextureRaw {
                texture_type: 0,
                cells: Vec::new(),
            },
        ));
        let idx = master.entries.len() - 1;
        self.dirty = true;
        self.selected_texture = Some(idx);
        self.selected_cell = None;
        Some(idx)
    }

    /// Duplicate the texture at `idx` (cell metadata + sheet pixels) under a new unique name.
    pub fn clone_texture(&mut self, game_path: &Path, idx: usize) -> Option<usize> {
        let (new_idx, src_name, new_name) = {
            let master = self.master.as_mut()?;
            let (src_bytes, src_tex) = master.entries.get(idx)?.clone();
            let src_name = String::from_utf8_lossy(&src_bytes).into_owned();
            let new_name = unique_texture_name(master, &format!("{}_copy", src_name));
            master
                .entries
                .push((new_name.clone().into_bytes(), src_tex));
            (master.entries.len() - 1, src_name, new_name)
        };

        // Give the clone its own sheet: copy the source override PNG if present, else decode the vanilla xnb into a fresh override PNG so the new texture renders standalone.
        let dst_png = Self::png_override_path(game_path, &new_name);
        if let Some(parent) = dst_png.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let src_png = Self::png_override_path(game_path, &src_name);
        if src_png.exists() {
            if let Err(e) = std::fs::copy(&src_png, &dst_png) {
                self.status = Some(format!("Cloned entry, but PNG copy failed: {}", e));
            }
        } else {
            let xnb = game_path
                .join("Content")
                .join("gfx")
                .join(format!("{}.xnb", src_name));
            if let Ok(img) = load_texture_from_path(xnb.to_str().unwrap_or("")) {
                let _ = img.save(&dst_png);
            }
        }

        self.dirty = true;
        self.selected_texture = Some(new_idx);
        self.selected_cell = None;
        Some(new_idx)
    }

    /// Remove the texture entry at `idx` from the working master and delete its PNG override.
    /// Saving writes the override master.zcm without the entry. Vanilla entries can be removed too.
    pub fn delete_texture(&mut self, game_path: &Path, idx: usize) {
        let name = self.texture_name(idx);
        if let Some(master) = self.master.as_mut() {
            if idx < master.entries.len() {
                master.entries.remove(idx);
            }
        }
        let png = Self::png_override_path(game_path, &name);
        if png.exists() {
            let _ = std::fs::remove_file(&png);
        }
        self.dirty = true;
        self.selected_texture = None;
        self.selected_cell = None;
        self.sheet_for = None;
    }

    /// Rename the texture at `idx` (master entry name + its PNG override file).
    /// Tracks the original vanilla name so the rename can be reverted.
    pub fn rename_texture(
        &mut self,
        game_path: &Path,
        idx: usize,
        new_name: &str,
    ) -> Result<(), String> {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            return Err("Name cannot be empty".to_string());
        }
        let old_name = self.texture_name(idx);
        if new_name == old_name {
            return Ok(());
        }

        let master = self.master.as_mut().ok_or("No master.zcm loaded")?;
        // Reject names already used by another entry.
        if master
            .entries
            .iter()
            .enumerate()
            .any(|(i, (n, _))| i != idx && String::from_utf8_lossy(n) == new_name)
        {
            return Err(format!("A texture named '{}' already exists", new_name));
        }
        let Some(entry) = master.entries.get_mut(idx) else {
            return Err("Invalid selection".to_string());
        };
        entry.0 = new_name.as_bytes().to_vec();

        // Track the original vanilla name for revert.
        let original = self.renamed_from.remove(&old_name).or_else(|| {
            self.vanilla_master
                .as_ref()
                .and_then(|v| v.get(&old_name).map(|_| old_name.clone()))
        });
        if let Some(orig) = original {
            if orig != new_name {
                self.renamed_from.insert(new_name.to_string(), orig);
            }
        }

        // Move the PNG override if present.
        let old_png = Self::png_override_path(game_path, &old_name);
        if old_png.exists() {
            let new_png = Self::png_override_path(game_path, new_name);
            if let Some(parent) = new_png.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::rename(&old_png, &new_png);
        }

        self.dirty = true;
        self.sheet_for = None;
        self.rename_buffer = new_name.to_string();
        Ok(())
    }

    /// If the texture at `idx` was renamed from a vanilla texture this session, the vanilla name.
    pub fn revert_name_target(&self, idx: usize) -> Option<String> {
        let name = self.texture_name(idx);
        self.renamed_from.get(&name).cloned()
    }

    /// Restore the texture at `idx` to its tracked vanilla name.
    pub fn revert_name(&mut self, game_path: &Path, idx: usize) -> Result<(), String> {
        let Some(orig) = self.revert_name_target(idx) else {
            return Ok(());
        };
        self.rename_texture(game_path, idx, &orig)
    }

    /// Restore the texture at `idx` to its vanilla cell metadata and remove its PNG override.
    pub fn reset_texture_to_vanilla(&mut self, game_path: &Path, idx: usize) -> Result<(), String> {
        let name = self.texture_name(idx);
        let van_tex = self
            .vanilla_master
            .as_ref()
            .and_then(|v| v.get(&name))
            .cloned()
            .ok_or("This texture has no vanilla version to reset to")?;

        if let Some(master) = self.master.as_mut() {
            if let Some(entry) = master.entries.get_mut(idx) {
                entry.1 = van_tex;
            }
        }

        let png = Self::png_override_path(game_path, &name);
        if png.exists() {
            std::fs::remove_file(&png)
                .map_err(|e| format!("Failed to remove PNG override: {}", e))?;
        }

        self.dirty = true;
        self.selected_cell = None;
        self.sheet_for = None; // force the preview to reload from vanilla
        Ok(())
    }

    /// Names of all textures, filtered by the search box (UTF-8 lossy view of the raw name).
    pub fn filtered_indices(&self) -> Vec<usize> {
        let Some(master) = &self.master else {
            return Vec::new();
        };
        let needle = self.search.to_lowercase();
        master
            .entries
            .iter()
            .enumerate()
            .filter(|(_, (name, _))| {
                needle.is_empty()
                    || String::from_utf8_lossy(name)
                        .to_lowercase()
                        .contains(&needle)
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn texture_name(&self, idx: usize) -> String {
        self.master
            .as_ref()
            .and_then(|m| m.entries.get(idx))
            .map(|(n, _)| String::from_utf8_lossy(n).into_owned())
            .unwrap_or_default()
    }

    /// Ensure the sheet handle matches the selected texture, loading it if needed.
    /// The override PNG takes priority over the vanilla xnb so the preview reflects pending pixel edits.
    pub fn ensure_sheet(&mut self, ctx: &egui::Context, game_path: &Path) {
        let Some(idx) = self.selected_texture else {
            return;
        };
        // `reload_sheet` always sets `sheet_for = Some(idx)`, so guarding on it alone also caches the failure case (no sheet found) and avoids re-decoding from disk every frame.
        if self.sheet_for == Some(idx) {
            return;
        }
        self.reload_sheet(ctx, game_path, idx);
    }

    /// Force-reload the sheet image for a texture index from disk.
    pub fn reload_sheet(&mut self, ctx: &egui::Context, game_path: &Path, idx: usize) {
        let name = self.texture_name(idx);
        let png = Self::png_override_path(game_path, &name);

        let img = if png.exists() {
            self.watch_path = Some(png.clone());
            self.watch_mtime = std::fs::metadata(&png).and_then(|m| m.modified()).ok();
            image::open(&png)
                .map(|i| i.to_rgba8())
                .map_err(|e| e.to_string())
        } else {
            self.watch_path = None;
            self.watch_mtime = None;
            let xnb = game_path
                .join("Content")
                .join("gfx")
                .join(format!("{}.xnb", name));
            load_texture_from_path(xnb.to_str().unwrap_or(""))
        };

        match img {
            Ok(img) => {
                let (w, h) = (img.width(), img.height());
                let ci = egui::ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    img.as_raw(),
                );
                let handle = ctx.load_texture(format!("tex_edit_{}", name), ci, Default::default());
                self.sheet_handle = Some(handle);
                self.sheet_size = (w, h);
                self.sheet_for = Some(idx);
            }
            Err(e) => {
                self.sheet_handle = None;
                self.sheet_for = Some(idx);
                self.sheet_size = (0, 0);
                self.status = Some(format!("No sheet for '{}': {}", name, e));
            }
        }
    }

    /// If the watched override PNG changed on disk (external edit), reload the preview.
    pub fn poll_external_edit(&mut self, ctx: &egui::Context, game_path: &Path) {
        let (Some(path), Some(idx)) = (self.watch_path.clone(), self.selected_texture) else {
            return;
        };
        let Ok(modified) = std::fs::metadata(&path).and_then(|m| m.modified()) else {
            return;
        };
        if self.watch_mtime != Some(modified) {
            self.watch_mtime = Some(modified);
            self.reload_sheet(ctx, game_path, idx);
            self.status = Some(format!(
                "Reloaded '{}' after external edit",
                self.texture_name(idx)
            ));
        }
    }

    /// Copy an imported image into the config mirror as `textures/<name>.png`, re-encoding to PNG so any supported input format works.
    /// This is the loader-visible pixel override.
    pub fn import_png(&mut self, game_path: &Path, idx: usize, src: &Path) -> Result<(), String> {
        let name = self.texture_name(idx);
        let dest = Self::png_override_path(game_path, &name);
        std::fs::create_dir_all(dest.parent().unwrap())
            .map_err(|e| format!("Failed to create textures dir: {}", e))?;
        let img = image::open(src).map_err(|e| format!("Failed to read image: {}", e))?;
        img.to_rgba8()
            .save(&dest)
            .map_err(|e| format!("Failed to write PNG: {}", e))?;
        Ok(())
    }

    /// Seed the override PNG from the vanilla sheet (if not present) and open it for external editing.
    /// When `editor` is non-empty it is launched with the PNG as its final argument, otherwise the OS default handler is used.
    pub fn open_external(
        &mut self,
        game_path: &Path,
        idx: usize,
        editor: &str,
    ) -> Result<PathBuf, String> {
        let name = self.texture_name(idx);
        let dest = Self::png_override_path(game_path, &name);
        if !dest.exists() {
            std::fs::create_dir_all(dest.parent().unwrap())
                .map_err(|e| format!("Failed to create textures dir: {}", e))?;
            let xnb = game_path
                .join("Content")
                .join("gfx")
                .join(format!("{}.xnb", name));
            let img = load_texture_from_path(xnb.to_str().unwrap_or(""))?;
            img.save(&dest)
                .map_err(|e| format!("Failed to seed PNG: {}", e))?;
        }

        let editor = editor.trim();
        if editor.is_empty() {
            open::that(&dest).map_err(|e| format!("Failed to open editor: {}", e))?;
        } else {
            std::process::Command::new(editor)
                .arg(&dest)
                .spawn()
                .map_err(|e| format!("Failed to launch '{}': {}", editor, e))?;
        }

        self.watch_path = Some(dest.clone());
        self.watch_mtime = std::fs::metadata(&dest).and_then(|m| m.modified()).ok();
        Ok(dest)
    }

    /// Persist edited cell metadata to the config-mirror master.zcm.
    pub fn save_master(&mut self, game_path: &Path) -> Result<(), String> {
        let master = self.master.as_ref().ok_or("No master.zcm loaded")?;
        let flag_defs = self.flag_defs.as_ref().ok_or("No flag defs loaded")?;
        let dest = Self::master_override_path(game_path);
        std::fs::create_dir_all(dest.parent().unwrap())
            .map_err(|e| format!("Failed to create gfx dir: {}", e))?;
        master
            .save_to_path(&dest, flag_defs)
            .map_err(|e| format!("Failed to write master.zcm: {}", e))?;
        self.dirty = false;
        Ok(())
    }
}

/// Return a texture name (UTF-8) not already used by any entry, appending _1, _2, ... on collision.
fn unique_texture_name(master: &MasterTextures, base: &str) -> String {
    let exists = |n: &str| {
        master
            .entries
            .iter()
            .any(|(nm, _)| String::from_utf8_lossy(nm) == n)
    };
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
