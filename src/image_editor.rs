use egui::TextureHandle;
use image::RgbaImage;
use image::imageops::FilterType;
use sas2_parser::xnb_loader::load_texture_from_path;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Editor state for the Images tab: add custom item icons so custom items don't reuse a vanilla
/// icon.
///
/// Vanilla icons live in `items.xnb` (a 32-col grid of 128x128 tiles, `LootDef.img`).
/// That atlas can't grow (a taller texture exceeds the runtime max), so custom icons live in a SEPARATE atlas (`custom_items.png`) at their own local indices, an item references one with `img = vanilla_capacity + local_index`, and the loader redirects those draws to the custom atlas.
/// Icons are edited in the working-assets folder and only composited into the live config (`textures/custom_items.png`) on Apply.
const COLS: u32 = 32;
const TILE: u32 = 128;

#[derive(Default)]
pub struct ImageEditor {
    pub loaded: bool,
    /// Vanilla atlas slot count (rows * 32). An item's custom icon is `capacity + local`.
    pub capacity: u32,
    pub vanilla_size: (u32, u32),
    /// (local index, preview handle) for each custom icon in the working folder.
    pub icons: Vec<(i32, TextureHandle)>,
    pub status: Option<String>,
}

impl ImageEditor {
    fn icons_dir() -> PathBuf {
        crate::assets::working_root().join(crate::assets::ICONS_DIR)
    }
    fn items_xnb(game_path: &Path) -> PathBuf {
        game_path.join("Content").join("gfx").join("items.xnb")
    }

    pub fn ensure_loaded(&mut self, ctx: &egui::Context, game_path: &Path) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        self.reload(ctx, game_path);
    }

    pub fn reload(&mut self, ctx: &egui::Context, game_path: &Path) {
        match load_texture_from_path(Self::items_xnb(game_path).to_str().unwrap_or("")) {
            Ok(img) => {
                self.vanilla_size = (img.width(), img.height());
                let rows = (img.height() / TILE).max(1);
                self.capacity = rows * COLS;
            }
            Err(e) => {
                self.status = Some(format!("Could not read items.xnb: {}", e));
                self.capacity = 1024;
            }
        }

        self.icons.clear();
        let mut found = icon_indices(&Self::icons_dir());
        found.sort();
        for index in found {
            let png = Self::icons_dir().join(format!("{}.png", index));
            if let Ok(img) = image::open(&png) {
                let rgba = img.to_rgba8();
                let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                let ci = egui::ColorImage::from_rgba_unmultiplied([w, h], rgba.as_raw());
                let handle =
                    ctx.load_texture(format!("custom_icon_{}", index), ci, Default::default());
                self.icons.push((index, handle));
            }
        }
    }

    /// Global `img` value an item should use to show the custom icon at `local`.
    pub fn global_img(&self, local: i32) -> i32 {
        self.capacity as i32 + local
    }

    /// Lowest free local index in the custom atlas (0..1023).
    pub fn next_free_local(&self) -> Option<i32> {
        let taken: HashSet<i32> = self.icons.iter().map(|(i, _)| *i).collect();
        (0..(COLS * COLS) as i32).find(|i| !taken.contains(i))
    }

    /// Import an image as the next free custom icon (resized to 128x128). Returns its local index.
    pub fn import_icon(
        &mut self,
        ctx: &egui::Context,
        game_path: &Path,
        src: &Path,
    ) -> Result<i32, String> {
        let local = self
            .next_free_local()
            .ok_or("No free custom-icon slots left")?;
        let dir = Self::icons_dir();
        std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create icons dir: {}", e))?;
        let img = image::open(src).map_err(|e| format!("Failed to read image: {}", e))?;
        let icon = image::imageops::resize(&img.to_rgba8(), TILE, TILE, FilterType::Lanczos3);
        icon.save(dir.join(format!("{}.png", local)))
            .map_err(|e| format!("Failed to write icon: {}", e))?;
        self.reload(ctx, game_path);
        Ok(local)
    }

    pub fn delete_icon(&mut self, ctx: &egui::Context, game_path: &Path, local: i32) {
        let png = Self::icons_dir().join(format!("{}.png", local));
        let _ = std::fs::remove_file(&png);
        self.reload(ctx, game_path);
    }
}

/// Local indices of every `<n>.png` in an icons folder.
pub fn icon_indices(dir: &Path) -> Vec<i32> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("png") {
                if let Some(i) = p
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .and_then(|s| s.parse().ok())
                {
                    out.push(i);
                }
            }
        }
    }
    out
}

/// Composite `icons_dir/<local>.png` files into a single custom atlas PNG (32-col grid, 128px tiles) at `dest`.
/// Used at Apply time on the merged icon set. No-op (removes dest) if empty.
pub fn build_custom_atlas(icons_dir: &Path, dest: &Path) -> Result<(), String> {
    let mut indices = icon_indices(icons_dir);
    indices.sort();
    if indices.is_empty() {
        let _ = std::fs::remove_file(dest);
        return Ok(());
    }
    let max = *indices.iter().max().unwrap() as u32;
    let rows = (max / COLS) + 1;
    let mut atlas = RgbaImage::new(COLS * TILE, rows * TILE);
    for local in indices {
        let png = icons_dir.join(format!("{}.png", local));
        if let Ok(img) = image::open(&png) {
            let icon = image::imageops::resize(&img.to_rgba8(), TILE, TILE, FilterType::Lanczos3);
            let x = ((local as u32) % COLS) * TILE;
            let y = ((local as u32) / COLS) * TILE;
            image::imageops::replace(&mut atlas, &icon, x as i64, y as i64);
        }
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create dir: {}", e))?;
    }
    atlas
        .save(dest)
        .map_err(|e| format!("Failed to write custom atlas: {}", e))?;
    Ok(())
}
