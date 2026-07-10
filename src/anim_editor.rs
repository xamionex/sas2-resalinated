use crate::atlas::assemble_frame;
use egui::TextureHandle;
use image::RgbaImage;
use sas2_parser::char_def::{Animation, CharDef, KeyFrame};
use sas2_parser::subflags::SubFlagDefCatalog;
use sas2_parser::xnb_loader::load_texture_from_path;
use sas2_parser::xtexture::XTextureMeta;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Editor state for the animation timeline tab (Phase 4).
///
/// Loads a character definition (`.zsx`), lets the user edit its animations (keyframe timing,
/// frame references, scripts) and the per-part transforms of each frame, previews any frame by
/// reusing the bestiary sprite assembly, and saves the result into the loader's config mirror at
/// `BepInEx/config/amione.SaS2Resalter/Character/data/<name>.zsx`.
#[derive(Default)]
pub struct AnimEditor {
    pub files: Vec<String>,
    pub files_loaded: bool,
    pub file_search: String,

    pub loaded_stem: Option<String>,
    pub char_def: Option<CharDef>,
    /// Pristine vanilla copy (game .zsx), used for reset-to-vanilla of individual animations.
    pub vanilla_char: Option<CharDef>,
    pub dirty: bool,

    pub selected_anim: Option<usize>,
    pub selected_kf: Option<usize>,
    pub selected_part: Option<usize>,

    /// Cell metadata for the character's sheet, keyed by texture name (lossy is fine for preview).
    tex_meta: Option<XTextureMeta>,
    sheet: Option<RgbaImage>,
    preview_handle: Option<TextureHandle>,
    preview_for: Option<usize>,
    preview_size: (u32, u32),

    pub playing: bool,
    play_ticks: f32,

    pub status: Option<String>,
}

impl AnimEditor {
    // Overrides live in the shared working-assets folder (snapshotted into presets on Save).
    fn config_root(_game_path: &Path) -> PathBuf {
        crate::assets::working_root()
    }

    fn override_zsx(game_path: &Path, stem: &str) -> PathBuf {
        Self::config_root(game_path)
            .join("Character/data")
            .join(format!("{}.zsx", stem))
    }

    fn game_zsx(game_path: &Path, stem: &str) -> PathBuf {
        game_path
            .join("Character/data")
            .join(format!("{}.zsx", stem))
    }

    /// List available character defs once (game files plus any override copies), by stem.
    pub fn ensure_files(&mut self, game_path: &Path) {
        if self.files_loaded {
            return;
        }
        self.files_loaded = true;

        let mut stems: Vec<String> = Vec::new();
        let push_dir = |dir: PathBuf, stems: &mut Vec<String>| {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.extension().and_then(|x| x.to_str()) == Some("zsx") {
                        if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                            stems.push(stem.to_string());
                        }
                    }
                }
            }
        };
        push_dir(game_path.join("Character/data"), &mut stems);
        push_dir(Self::config_root(game_path).join("Character/data"), &mut stems);
        stems.sort();
        stems.dedup();
        self.files = stems;
    }

    pub fn filtered_files(&self) -> Vec<String> {
        let needle = self.file_search.to_lowercase();
        self.files
            .iter()
            .filter(|s| needle.is_empty() || s.to_lowercase().contains(&needle))
            .cloned()
            .collect()
    }

    /// Load a character def (override copy preferred) plus its sheet and cell metadata for preview.
    pub fn load_char(&mut self, game_path: &Path, stem: &str) {
        let path = {
            let o = Self::override_zsx(game_path, stem);
            if o.exists() {
                o
            } else {
                Self::game_zsx(game_path, stem)
            }
        };

        match CharDef::load_from_path(&path) {
            Ok(cd) => {
                self.load_preview_assets(game_path, &cd.tex_name);
                // Keep a pristine vanilla copy (always from the game's own file) for reset.
                self.vanilla_char = CharDef::load_from_path(&Self::game_zsx(game_path, stem)).ok();
                self.char_def = Some(cd);
                self.loaded_stem = Some(stem.to_string());
                self.dirty = false;
                self.selected_anim = Some(0);
                self.selected_kf = None;
                self.selected_part = None;
                self.preview_for = None;
                self.preview_handle = None;
                self.playing = false;
                self.play_ticks = 0.0;
                self.status = Some(format!("Loaded {}", stem));
            }
            Err(e) => self.status = Some(format!("Failed to load {}: {}", stem, e)),
        }
    }

    fn load_preview_assets(&mut self, game_path: &Path, tex_name: &str) {
        // Sheet pixels: prefer the PNG override (so texture edits show), else the vanilla xnb.
        let png = Self::config_root(game_path)
            .join("textures")
            .join(format!("{}.png", tex_name));
        self.sheet = if png.exists() {
            image::open(&png).ok().map(|i| i.to_rgba8())
        } else {
            let xnb = game_path
                .join("Content/gfx")
                .join(format!("{}.xnb", tex_name));
            load_texture_from_path(xnb.to_str().unwrap_or("")).ok()
        };

        // Cell metadata from master.zcm (override preferred).
        let flagdefs = game_path.join("Content/gfx/flagdefs.zfd");
        let master_override = Self::config_root(game_path).join("Content/gfx/master.zcm");
        let master = if master_override.exists() {
            master_override
        } else {
            game_path.join("Content/gfx/master.zcm")
        };
        self.tex_meta = match SubFlagDefCatalog::load_from_path(&flagdefs) {
            Ok(defs) => XTextureMeta::load_all_from_master_path(&master, &defs)
                .ok()
                .and_then(|mut m: HashMap<String, XTextureMeta>| m.remove(tex_name)),
            Err(_) => None,
        };
    }

    /// The frame index displayed for the current selection (selected keyframe's frame_ref).
    pub fn current_frame_index(&self) -> Option<usize> {
        let cd = self.char_def.as_ref()?;
        let anim = cd.animations.get(self.selected_anim?)?;
        let kf = anim.key_frames.get(self.selected_kf?)?;
        let idx = kf.frame_ref;
        if idx >= 0 && (idx as usize) < cd.frames.len() {
            Some(idx as usize)
        } else {
            None
        }
    }

    /// Rebuild the preview texture for the given frame index if it is not already current.
    pub fn ensure_preview(&mut self, ctx: &egui::Context, frame_idx: usize) {
        // Guard on `preview_for` alone so a frame that fails to assemble is not retried every
        // frame; `invalidate_preview` resets it after edits to force a rebuild.
        if self.preview_for == Some(frame_idx) {
            return;
        }
        self.preview_for = Some(frame_idx);
        self.preview_handle = None;

        let (Some(cd), Some(sheet)) = (self.char_def.as_ref(), self.sheet.as_ref()) else {
            return;
        };
        let Some(frame) = cd.frames.get(frame_idx) else {
            return;
        };
        if let Some(img) = assemble_frame(frame, sheet, self.tex_meta.as_ref()) {
            let (w, h) = (img.width(), img.height());
            let ci = egui::ColorImage::from_rgba_unmultiplied(
                [w as usize, h as usize],
                img.as_raw(),
            );
            self.preview_handle =
                Some(ctx.load_texture("anim_preview", ci, Default::default()));
            self.preview_size = (w, h);
        }
    }

    pub fn preview(&self) -> Option<(&TextureHandle, (u32, u32))> {
        self.preview_handle.as_ref().map(|h| (h, self.preview_size))
    }

    /// Invalidate the cached preview (call after editing parts/frames).
    pub fn invalidate_preview(&mut self) {
        self.preview_for = None;
    }

    /// Advance playback across the selected animation's keyframes (durations are in ticks).
    /// Returns the keyframe index to display, updating `selected_kf` as a side effect.
    pub fn advance_playback(&mut self, dt: f32) {
        if !self.playing {
            return;
        }
        let Some(cd) = self.char_def.as_ref() else {
            return;
        };
        let Some(anim) = self.selected_anim.and_then(|i| cd.animations.get(i)) else {
            return;
        };
        let total: i64 = anim.key_frames.iter().map(|k| k.duration.max(1) as i64).sum();
        if total <= 0 || anim.key_frames.is_empty() {
            return;
        }

        // 60 ticks per second matches the engine's fixed step closely enough for preview.
        self.play_ticks += dt * 60.0;
        let mut t = (self.play_ticks as i64) % total;
        let mut idx = 0;
        for (i, kf) in anim.key_frames.iter().enumerate() {
            let d = kf.duration.max(1) as i64;
            if t < d {
                idx = i;
                break;
            }
            t -= d;
        }
        self.selected_kf = Some(idx);
    }

    /// True when the animation at `anim_idx` exists in the vanilla char def (can be reset).
    pub fn is_vanilla_anim(&self, anim_idx: usize) -> bool {
        let Some(name) = self
            .char_def
            .as_ref()
            .and_then(|cd| cd.animations.get(anim_idx))
            .map(|a| a.name.as_str())
        else {
            return false;
        };
        self.vanilla_char
            .as_ref()
            .map_or(false, |v| v.animations.iter().any(|a| a.name == name))
    }

    /// Restore the animation at `anim_idx`'s keyframes from vanilla (matched by name).
    pub fn reset_anim_to_vanilla(&mut self, anim_idx: usize) -> Result<(), String> {
        let name = self
            .char_def
            .as_ref()
            .and_then(|cd| cd.animations.get(anim_idx))
            .map(|a| a.name.clone())
            .ok_or("No animation selected")?;
        let van_anim = self
            .vanilla_char
            .as_ref()
            .and_then(|v| v.animations.iter().find(|a| a.name == name))
            .cloned()
            .ok_or("This animation has no vanilla version to reset to")?;
        if let Some(cd) = self.char_def.as_mut() {
            if let Some(a) = cd.animations.get_mut(anim_idx) {
                *a = van_anim;
            }
        }
        self.dirty = true;
        self.selected_kf = None;
        self.invalidate_preview();
        Ok(())
    }

    /// Duplicate the animation at `anim_idx` under a new unique "<name>_copy" name.
    pub fn clone_anim(&mut self, anim_idx: usize) -> Option<usize> {
        let cd = self.char_def.as_mut()?;
        let mut clone = cd.animations.get(anim_idx)?.clone();
        clone.name = unique_anim_name(cd, &format!("{}_copy", clone.name));
        cd.animations.push(clone);
        self.dirty = true;
        Some(cd.animations.len() - 1)
    }

    /// Add a blank animation (no keyframes). Returns the new index.
    pub fn add_blank_anim(&mut self) -> Option<usize> {
        let cd = self.char_def.as_mut()?;
        let name = unique_anim_name(cd, "new_anim");
        cd.animations.push(Animation {
            name,
            key_frames: Vec::new(),
        });
        self.dirty = true;
        Some(cd.animations.len() - 1)
    }

    /// Add a simple looping animation preset that steps through the first few frames.
    pub fn add_loop_preset(&mut self) -> Option<usize> {
        let cd = self.char_def.as_mut()?;
        let name = unique_anim_name(cd, "new_loop");
        let frame_count = cd.frames.len() as i32;
        let mut key_frames = Vec::new();
        if frame_count > 0 {
            let n = frame_count.min(4);
            for i in 0..n {
                key_frames.push(KeyFrame {
                    frame_ref: i,
                    duration: 6,
                    lerp: true,
                    scripts: Vec::new(),
                });
            }
        }
        cd.animations.push(Animation { name, key_frames });
        self.dirty = true;
        Some(cd.animations.len() - 1)
    }

    /// Save the working char def to the config override mirror.
    pub fn save(&mut self, game_path: &Path) -> Result<(), String> {
        let cd = self.char_def.as_ref().ok_or("No character loaded")?;
        let stem = self.loaded_stem.as_ref().ok_or("No file loaded")?;
        let dest = Self::override_zsx(game_path, stem);
        std::fs::create_dir_all(dest.parent().unwrap())
            .map_err(|e| format!("Failed to create data dir: {}", e))?;
        cd.save_to_path(&dest)
            .map_err(|e| format!("Failed to write {}.zsx: {}", stem, e))?;
        self.dirty = false;
        Ok(())
    }
}

/// Return an animation name not already used in the char def, appending _1, _2, ... on collision.
fn unique_anim_name(cd: &CharDef, base: &str) -> String {
    let exists = |n: &str| cd.animations.iter().any(|a| a.name == n);
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
