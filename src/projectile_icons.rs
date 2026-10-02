//! Icon cache for the Projectiles tab.
//!
//! Magic projectiles are drawn by the game from `Content/gfx/sprites.xnb` (the particle sheet), so their tiles use the same cells the particle classes draw (the cell is picked per type below).
//! Weapon and thrown projectiles use cell 0 of their loot texture sheet, which is the sprite the game draws in flight.

use egui::TextureHandle;
use image::RgbaImage;
use sas2_parser::subflags::SubFlagDefCatalog;
use sas2_parser::xnb_loader::load_texture_from_path;
use sas2_parser::xtexture::XTextureMeta;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Where a projectile icon comes from.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum IconSource {
    /// A cell of the particle sprite sheet.
    ParticleCell(i32),
    /// Cell 0 of the named loot texture sheet (Content/gfx/<name>.xnb).
    LootTexture(&'static str),
    /// An icon of the item atlas (items.xnb, a 32 wide grid of 128x128 tiles).
    ItemIcon(i32),
}

/// Icon edge length before upload (cropped cells are upscaled to this with nearest neighbour).
const ICON_SIZE: u32 = 48;

/// Items atlas layout (same as the Items tab).
const ITEM_TILE: u32 = 128;
const ITEM_COLUMNS: u32 = 32;

pub struct ProjectileIcons {
    game_path: PathBuf,
    /// Particle sheet pixels and the master.zcm cell rects for the "sprites" texture.
    sprites: Option<(RgbaImage, Vec<Option<(i32, i32, i32, i32)>>)>,
    /// Loot texture sheets by name, with their cell 0 rect when the metadata has one.
    loot: HashMap<&'static str, Option<(RgbaImage, (i32, i32, i32, i32))>>,
    /// Item atlas pixels (items.xnb), loaded on demand.
    items: Option<RgbaImage>,
    textures: HashMap<IconSource, TextureHandle>,
}

impl ProjectileIcons {
    pub fn new(game_path: &Path) -> Self {
        Self {
            game_path: game_path.to_path_buf(),
            sprites: None,
            loot: HashMap::new(),
            items: None,
            textures: HashMap::new(),
        }
    }

    /// Loads the item atlas pixels once (used for weapon and thrown item icons).
    fn ensure_items(&mut self) {
        if self.items.is_some() {
            return;
        }
        let path = self.game_path.join("Content").join("gfx").join("items.xnb");
        let Some(path) = path.to_str() else { return };
        match load_texture_from_path(path) {
            Ok(img) => self.items = Some(img),
            Err(e) => eprintln!("[projectile_icons] items.xnb: {}", e),
        }
    }

    /// Loads the particle sheet and its cell metadata once.
    fn ensure_sprites(&mut self) {
        if self.sprites.is_some() {
            return;
        }
        let gfx = self.game_path.join("Content").join("gfx");
        let flag_defs = match SubFlagDefCatalog::load_from_path(&gfx.join("flagdefs.zfd")) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("[projectile_icons] flagdefs.zfd: {}", e);
                return;
            }
        };
        let meta = match XTextureMeta::load_all_from_master_path(&gfx.join("master.zcm"), &flag_defs)
        {
            Ok(m) => m,
            Err(e) => {
                eprintln!("[projectile_icons] master.zcm: {}", e);
                return;
            }
        };
        let cells = meta
            .get("sprites")
            .map(|t| t.cells.iter().map(|c| c.as_ref().map(|c| c.src_rect)).collect())
            .unwrap_or_default();
        let Some(path) = gfx.join("sprites.xnb").to_str().map(str::to_string) else {
            return;
        };
        match load_texture_from_path(&path) {
            Ok(img) => self.sprites = Some((img, cells)),
            Err(e) => eprintln!("[projectile_icons] sprites.xnb: {}", e),
        }
    }

    /// Loads a loot texture sheet (cell 0 is used).
    fn ensure_loot(&mut self, name: &'static str) {
        if self.loot.contains_key(name) {
            return;
        }
        let gfx = self.game_path.join("Content").join("gfx");
        let flag_defs = match SubFlagDefCatalog::load_from_path(&gfx.join("flagdefs.zfd")) {
            Ok(d) => d,
            Err(_) => {
                self.loot.insert(name, None);
                return;
            }
        };
        let rect = XTextureMeta::load_all_from_master_path(&gfx.join("master.zcm"), &flag_defs)
            .ok()
            .and_then(|m| {
                m.get(name)
                    .and_then(|t| t.cells.first())
                    .and_then(|c| c.as_ref().map(|c| c.src_rect))
            });
        let Some((img, rect)) = (|| {
            let path = gfx.join(format!("{}.xnb", name));
            let img = load_texture_from_path(path.to_str()?).ok()?;
            let rect = rect.unwrap_or((0, 0, img.width() as i32, img.height() as i32));
            Some((img, rect))
        })() else {
            self.loot.insert(name, None);
            return;
        };
        self.loot.insert(name, Some((img, rect)));
    }

    /// Crops the icon out of its sheet, trims transparent padding and scales it to `ICON_SIZE`.
    fn crop(&mut self, source: IconSource) -> Option<RgbaImage> {
        let (sheet, rect) = match source {
            IconSource::ParticleCell(cell) => {
                self.ensure_sprites();
                let (sheet, cells) = self.sprites.as_ref()?;
                let rect = cells.get(cell.max(0) as usize).copied().flatten()?;
                (sheet, rect)
            }
            IconSource::LootTexture(name) => {
                self.ensure_loot(name);
                let (sheet, rect) = self.loot.get(&name)?.as_ref()?;
                (sheet, *rect)
            }
            IconSource::ItemIcon(img) => {
                if img < 0 {
                    return None;
                }
                self.ensure_items();
                let sheet = self.items.as_ref()?;
                let col = (img as u32) % ITEM_COLUMNS;
                let row = (img as u32) / ITEM_COLUMNS;
                (
                    sheet,
                    (
                        (col * ITEM_TILE) as i32,
                        (row * ITEM_TILE) as i32,
                        ITEM_TILE as i32,
                        ITEM_TILE as i32,
                    ),
                )
            }
        };

        let (sx, sy, sw, sh) = rect;
        if sw <= 0 || sh <= 0 {
            return None;
        }
        // Trim fully transparent padding so small sprites in big cells still read at tile size.
        let (mut left, mut top, mut right, mut bottom) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
        for y in 0..sh {
            for x in 0..sw {
                let px = sx + x;
                let py = sy + y;
                if px < 0 || py < 0 || px as u32 >= sheet.width() || py as u32 >= sheet.height() {
                    continue;
                }
                if sheet.get_pixel(px as u32, py as u32)[3] > 0 {
                    left = left.min(x);
                    top = top.min(y);
                    right = right.max(x);
                    bottom = bottom.max(y);
                }
            }
        }
        if left == i32::MAX {
            return None; // fully transparent cell
        }
        let crop_w = right - left + 1;
        let crop_h = bottom - top + 1;

        let mut out = RgbaImage::new(ICON_SIZE, ICON_SIZE);
        // Fit the trimmed sprite into the icon box, centered, as large as possible.
        let scale = (ICON_SIZE as f32 / crop_w as f32).min(ICON_SIZE as f32 / crop_h as f32);
        let dw = ((crop_w as f32 * scale).round() as u32).max(1);
        let dh = ((crop_h as f32 * scale).round() as u32).max(1);
        let off_x = (ICON_SIZE - dw) / 2;
        let off_y = (ICON_SIZE - dh) / 2;
        for y in 0..dh {
            for x in 0..dw {
                let src_x = sx + left + (x as f32 / scale) as i32;
                let src_y = sy + top + (y as f32 / scale) as i32;
                if src_x < 0
                    || src_y < 0
                    || src_x as u32 >= sheet.width()
                    || src_y as u32 >= sheet.height()
                {
                    continue;
                }
                let p = *sheet.get_pixel(src_x as u32, src_y as u32);
                if p[3] == 0 {
                    continue;
                }
                out.put_pixel(off_x + x, off_y + y, p);
            }
        }
        Some(out)
    }

    /// Icon texture for a projectile, built once.
    pub fn texture(
        &mut self,
        ctx: &egui::Context,
        source: IconSource,
    ) -> Option<TextureHandle> {
        if !self.textures.contains_key(&source) {
            let img = self.crop(source)?;
            let (w, h) = (img.width() as usize, img.height() as usize);
            let pixels = img.as_raw().clone();
            let color_image = egui::ColorImage::from_rgba_unmultiplied([w, h], &pixels);
            let handle = ctx.load_texture(
                format!("projectile_icon_{:?}", source),
                color_image,
                Default::default(),
            );
            self.textures.insert(source, handle);
        }
        self.textures.get(&source).cloned()
    }
}
