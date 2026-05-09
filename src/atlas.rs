use egui::{pos2, Rect, TextureHandle};
use sas2_save::loot_catalog::LootDef;
use sas2_save::xnb_loader::load_texture_from_path;
use std::path::Path;

pub struct ItemAtlas {
    pub texture: TextureHandle,
    pub width: u32,
    pub height: u32,
}

impl ItemAtlas {
    pub fn load(game_path: &Path, ctx: &egui::Context) -> Result<Self, String> {
        let path = game_path.join("Content").join("gfx").join("items.xnb");
        if !path.exists() {
            return Err(format!("items.xnb not found at {}", path.display()));
        }
        let img = load_texture_from_path(path.to_str().unwrap())?;
        let width = img.width();
        let height = img.height();
        let pixels = img.into_vec();
        let size = [width as usize, height as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        let texture = ctx.load_texture("items_atlas", color_image, Default::default());

        Ok(Self { texture, width, height })
    }

    pub fn icon_uv(&self, def: &LootDef) -> Option<Rect> {
        if def.img < 0 {
            return None;
        }
        let x = (def.img as u32 % 32) * 128;
        let y = (def.img as u32 / 32) * 128;
        let w = self.width as f32;
        let h = self.height as f32;
        Some(Rect::from_min_max(
            pos2(x as f32 / w, y as f32 / h),
            pos2((x + 128) as f32 / w, (y + 128) as f32 / h),
        ))
    }
}