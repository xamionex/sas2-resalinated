use egui::{pos2, Rect, TextureHandle};
use sas2_parser::loot_catalog::LootDef;
use sas2_parser::xnb_loader::load_texture_from_path;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::thread;

/// The item icon atlas loaded from items.xnb.
/// Icons are arranged in a 32-wide grid of 128×128 tiles.
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

        Ok(Self {
            texture,
            width,
            height,
        })
    }

    /// Returns the UV rect for the given loot definition's icon, or None if the item has no icon (img < 0).
    pub fn icon_uv(&self, def: &LootDef) -> Option<Rect> {
        if def.img < 0 {
            return None;
        }
        // Icons sit on a 32-wide grid of 128×128 tiles
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

/// Maximum number of GPU texture uploads per frame.
const MAX_UPLOADS_PER_FRAME: usize = 1;

pub struct MonsterTextureCache {
    /// Fully loaded GPU textures (created on UI thread).
    textures: HashMap<String, TextureHandle>,
    /// Indicates whether the background thread has finished loading.
    loading_complete: bool,
    pending_receiver: Option<mpsc::Receiver<(String, Vec<u8>, u32, u32)>>,
    total_textures: usize,
    loaded_textures: usize,
}

impl MonsterTextureCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            loading_complete: false,
            pending_receiver: None,
            total_textures: 0,
            loaded_textures: 0,
        }
    }

    pub fn is_loading(&self) -> bool {
        !self.loading_complete && self.pending_receiver.is_some()
    }

    /// Kick off a background thread that loads all unique texture filenames.
    pub fn start_preload(&mut self, game_path: &Path, texture_names: Vec<String>) {
        // Collect unique names
        let mut unique = texture_names;
        unique.sort();
        unique.dedup();
        self.total_textures = unique.len();

        let (tx, rx) = mpsc::channel();
        self.pending_receiver = Some(rx);

        let game_path_owned = game_path.to_path_buf();
        thread::spawn(move || {
            for name in unique {
                let path = game_path_owned
                    .join("Content")
                    .join("gfx")
                    .join(format!("{}.xnb", name));
                if !path.exists() {
                    continue;
                }
                if let Ok(img) = load_texture_from_path(path.to_str().unwrap()) {
                    let width = img.width();
                    let height = img.height();
                    let data = img.into_vec();  // consumes img
                    // Send to UI thread via channel
                    if tx.send((name, data, width, height)).is_err() {
                        break;
                    }
                }
            }
        });

        // We'll receive the raw data on the UI thread later.
        // Store the receiver so we can poll it each frame.
    }

    /// Call every frame on the UI thread to process any completed loads.
    /// A maximum of MAX_UPLOADS_PER_FRAME textures are uploaded to the GPU.
    pub fn update(&mut self, _ctx: &egui::Context) {
        let mut processed = 0;
        if let Some(rx) = &self.pending_receiver {
            loop {
                if processed >= MAX_UPLOADS_PER_FRAME {
                    break;
                }
                match rx.try_recv() {
                    Ok((name, data, w, h)) => {
                        processed += 1;
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            [w as usize, h as usize],
                            &data,
                        );
                        let tex = _ctx.load_texture(
                            format!("monster_tex_{}", name),
                            color_image,
                            Default::default(),
                        );
                        self.textures.insert(name, tex);
                        self.loaded_textures += 1;
                    }
                    Err(mpsc::TryRecvError::Empty) => break,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.loading_complete = true;
                        self.pending_receiver = None;
                        break;
                    }
                }
            }
        }
    }

    /// Progress as (loaded, total). Returns None if nothing is loading.
    pub fn progress(&self) -> Option<(usize, usize)> {
        if !self.loading_complete && self.pending_receiver.is_some() && self.total_textures > 0 {
            Some((self.loaded_textures, self.total_textures))
        } else {
            None
        }
    }

    /// Get a texture handle for the given monster, returns an owned handle.
    /// Textures are already uploaded during `update`, so this is a simple lookup.
    pub fn get_or_load(
        &self,
        texture_name: &str,
    ) -> Option<TextureHandle> {
        self.textures.get(texture_name).cloned()
    }
}

/// Returns the UV rectangle for the first 128×128 tile of a monster sprite sheet.
/// Falls back to (0,0)-(1,1) if the texture is smaller than 128 in either dimension.
pub fn monster_idle_uv(texture_width: u32, texture_height: u32) -> Rect {
    let frame_w = 128.min(texture_width);
    let frame_h = 128.min(texture_height);
    let x = 0.0;
    let y = 0.0;
    let w = frame_w as f32 / texture_width.max(1) as f32;
    let h = frame_h as f32 / texture_height.max(1) as f32;
    Rect::from_min_max(
        pos2(x, y),
        pos2(x + w, y + h),
    )
}