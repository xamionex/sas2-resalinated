use egui::{pos2, Rect, TextureHandle};
use image::RgbaImage;
use sas2_parser::char_def::CharDef;
use sas2_parser::loot_catalog::LootDef;
use sas2_parser::subflags::SubFlagDefCatalog;
use sas2_parser::xnb_loader::load_texture_from_path;
use sas2_parser::xtexture::XTextureMeta;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
        self.icon_uv_for_img(def.img)
    }

    /// UV rect for a raw icon index in the vanilla atlas, or None if negative or beyond the atlas
    /// (a custom-icon index).
    pub fn icon_uv_for_img(&self, img: i32) -> Option<Rect> {
        if img < 0 {
            return None;
        }
        // Icons sit on a 32-wide grid of 128×128 tiles.
        let x = (img as u32 % 32) * 128;
        let y = (img as u32 / 32) * 128;
        if y + 128 > self.height {
            return None;
        }
        let w = self.width as f32;
        let h = self.height as f32;
        Some(Rect::from_min_max(
            pos2(x as f32 / w, y as f32 / h),
            pos2((x + 128) as f32 / w, (y + 128) as f32 / h),
        ))
    }
}

const MAX_UPLOADS_PER_FRAME: usize = 4;

/// An assembled idle sprite plus the in-image location of the character origin, used to draw the
/// hitbox overlay to scale around the rendered body.
#[derive(Clone)]
pub struct HitboxPreview {
    pub handle: TextureHandle,
    /// Character-space origin (0,0) position within the image, in pixels.
    pub origin: (f32, f32),
    pub size: (u32, u32),
}

pub struct MonsterTextureCache {
    textures: HashMap<String, TextureHandle>,
    loading_complete: bool,
    pending_receiver: Option<mpsc::Receiver<(String, Vec<u8>, u32, u32)>>,
    total_textures: usize,
    loaded_textures: usize,
    assembled_cache: HashMap<(String, String), TextureHandle>,
    hitbox_cache: HashMap<(String, String), HitboxPreview>,
    game_path: Option<PathBuf>,
    xtexture_meta: Option<HashMap<String, XTextureMeta>>,
}

impl MonsterTextureCache {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
            loading_complete: false,
            pending_receiver: None,
            total_textures: 0,
            loaded_textures: 0,
            assembled_cache: HashMap::new(),
            hitbox_cache: HashMap::new(),
            game_path: None,
            xtexture_meta: None,
        }
    }

    pub(crate) fn set_game_path(&mut self, game_path: &Path) {
        self.game_path = Some(game_path.to_path_buf());
        // Drop caches tied to the previous game path.
        self.assembled_cache.clear();
        self.hitbox_cache.clear();

        // Load sprite-cell metadata so assemble_monster_sprite can use the real
        // srcRect / origin for each tile rather than assuming a 128×128 grid.
        let flagdefs_path = game_path.join("Content").join("gfx").join("flagdefs.zfd");
        let master_path = game_path.join("Content").join("gfx").join("master.zcm");

        match SubFlagDefCatalog::load_from_path(&flagdefs_path) {
            Err(e) => eprintln!("[atlas] Failed to load flagdefs.zfd: {}", e),
            Ok(flag_defs) => {
                match XTextureMeta::load_all_from_master_path(&master_path, &flag_defs) {
                    Err(e) => eprintln!("[atlas] Failed to load master.zcm: {}", e),
                    Ok(meta_map) => self.xtexture_meta = Some(meta_map),
                }
            }
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
                    let data = img.into_vec(); // consumes img
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

    pub fn update(&mut self, ctx: &egui::Context) {
        let mut processed = 0;
        if let Some(rx) = &self.pending_receiver {
            loop {
                if processed >= MAX_UPLOADS_PER_FRAME {
                    break;
                }
                match rx.try_recv() {
                    Ok((name, data, w, h)) => {
                        processed += 1;
                        let ci = egui::ColorImage::from_rgba_unmultiplied(
                            [w as usize, h as usize],
                            &data,
                        );
                        let tex = ctx.load_texture(
                            format!("monster_tex_{}", name),
                            ci,
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

    pub fn get_or_assemble(
        &mut self,
        ctx: &egui::Context,
        def_name: &str,
        texture_name: &str,
    ) -> Option<TextureHandle> {
        let game_path = self.game_path.as_ref()?.clone();
        let key = (def_name.to_string(), texture_name.to_string());

        if let Some(tex) = self.assembled_cache.get(&key) {
            return Some(tex.clone());
        }

        let tex_meta = self
            .xtexture_meta
            .as_ref()
            .and_then(|m| m.get(texture_name));

        match assemble_monster_sprite(&game_path, def_name, texture_name, tex_meta) {
            Some(img) => {
                let (w, h) = (img.width() as usize, img.height() as usize);
                let pixels = img.into_vec();
                let ci = egui::ColorImage::from_rgba_unmultiplied([w, h], &pixels);
                let tex = ctx.load_texture(
                    format!("assembled_{}_{}", def_name, texture_name),
                    ci,
                    Default::default(),
                );
                self.assembled_cache.insert(key, tex.clone());
                Some(tex)
            }
            None => {
                eprintln!(
                    "Failed to assemble monster sprite for {}/{}, falling back to raw first tile",
                    def_name, texture_name
                );
                let raw = self.textures.get(texture_name)?;
                eprintln!("Returning raw texture (full sheet) as fallback");
                Some(raw.clone())
            }
        }
    }

    /// Assemble (and cache) the monster's idle sprite together with its origin, for the hitbox
    /// overlay. Prefers a PNG texture override so pixel edits are reflected, else the vanilla xnb.
    pub fn get_idle_with_origin(
        &mut self,
        ctx: &egui::Context,
        def_name: &str,
        texture_name: &str,
    ) -> Option<HitboxPreview> {
        let game_path = self.game_path.as_ref()?.clone();
        let key = (def_name.to_string(), texture_name.to_string());
        if let Some(p) = self.hitbox_cache.get(&key) {
            return Some(p.clone());
        }

        let zsx = game_path
            .join("Character")
            .join("data")
            .join(format!("{}.zsx", def_name));
        let char_def = CharDef::load_from_path(&zsx).ok()?;
        let frame = char_def.idle_frame()?;

        let png = game_path
            .join("BepInEx/config/amione.SaS2Resalter/textures")
            .join(format!("{}.png", texture_name));
        let sheet = if png.exists() {
            image::open(&png).ok()?.to_rgba8()
        } else {
            let xnb = game_path
                .join("Content")
                .join("gfx")
                .join(format!("{}.xnb", texture_name));
            load_texture_from_path(xnb.to_str().unwrap_or("")).ok()?
        };

        let tex_meta = self.xtexture_meta.as_ref().and_then(|m| m.get(texture_name));
        let (img, origin) = assemble_frame_with_origin(frame, &sheet, tex_meta)?;
        let (w, h) = (img.width(), img.height());
        let pixels = img.into_vec();
        let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &pixels);
        let handle = ctx.load_texture(
            format!("hitbox_{}_{}", def_name, texture_name),
            ci,
            Default::default(),
        );
        let preview = HitboxPreview {
            handle,
            origin,
            size: (w, h),
        };
        self.hitbox_cache.insert(key, preview.clone());
        Some(preview)
    }
}

// Sprites

struct PartInfo {
    src_x: i32,
    src_y: i32,
    src_w: i32,
    src_h: i32,
    anchor_x: f32,
    anchor_y: f32,
    cx: f32,
    cy: f32,
    rot: f32,
    scale_x: f32,
    scale_y: f32,
    flip: i32,
}

pub fn assemble_monster_sprite(
    game_path: &Path,
    def_name: &str,
    texture_name: &str,
    tex_meta: Option<&XTextureMeta>,
) -> Option<RgbaImage> {
    // 1. Character definition
    let zsx_path = game_path
        .join("Character")
        .join("data")
        .join(format!("{}.zsx", def_name));

    let char_def = match CharDef::load_from_path(&zsx_path) {
        Ok(cd) => cd,
        Err(e) => {
            eprintln!("[assemble] Failed to load char def for {}: {}", def_name, e);
            return None;
        }
    };

    // 2. Idle frame
    let frame = match char_def.idle_frame() {
        Some(f) => f,
        None => {
            eprintln!("[assemble] No idle frame for {}", def_name);
            return None;
        }
    };

    // 3. Sprite sheet
    let tex_path = game_path
        .join("Content")
        .join("gfx")
        .join(format!("{}.xnb", texture_name));

    let sheet = match load_texture_from_path(tex_path.to_str().unwrap_or("")) {
        Ok(img) => img,
        Err(e) => {
            eprintln!("[assemble] Failed to load texture {}: {}", texture_name, e);
            return None;
        }
    };

    assemble_frame(frame, &sheet, tex_meta)
}

/// Composite a single character frame onto a tightly-cropped RGBA canvas.
///
/// Shared by the bestiary preview (idle frame) and the animation editor (any frame). Uses the
/// XTexture cell rect/origin when available, otherwise falls back to a 128x128 grid layout.
pub fn assemble_frame(
    frame: &sas2_parser::char_def::Frame,
    sheet: &RgbaImage,
    tex_meta: Option<&XTextureMeta>,
) -> Option<RgbaImage> {
    assemble_frame_with_origin(frame, sheet, tex_meta).map(|(img, _)| img)
}

/// Like [`assemble_frame`], but also returns where the character-space origin (0,0) lands inside
/// the cropped canvas, in pixels. The hitbox overlay uses this to align the box (which is centered
/// on that same origin) with the rendered sprite at a 1:1 world-pixel scale.
pub fn assemble_frame_with_origin(
    frame: &sas2_parser::char_def::Frame,
    sheet: &RgbaImage,
    tex_meta: Option<&XTextureMeta>,
) -> Option<(RgbaImage, (f32, f32))> {
    const BODY_MAX: i32 = 384;

    let sheet_w = sheet.width() as i32;
    let sheet_h = sheet.height() as i32;
    let fallback_cols = (sheet_w / 128).max(1);

    // 4. Compute Absolute Hierarchical Transforms
    let mut transforms = vec![None; frame.parts.len()];

    fn compute_transform(
        idx: usize,
        parts: &[sas2_parser::char_def::Part],
        transforms: &mut Vec<Option<(f32, f32, f32)>>,
    ) -> (f32, f32, f32) {
        if let Some(t) = transforms[idx] {
            return t;
        }
        let part = &parts[idx];
        if part.parent > -1 && (part.parent as usize) < parts.len() {
            let parent_idx = part.parent as usize;
            let (px, py, prot) = compute_transform(parent_idx, parts, transforms);

            let ox = part.parent_loc_offset.0;
            let oy = part.parent_loc_offset.1;

            let x = px + prot.cos() * ox + (prot + std::f32::consts::FRAC_PI_2).cos() * oy;
            let y = py + prot.sin() * ox + (prot + std::f32::consts::FRAC_PI_2).sin() * oy;
            let rot = prot + part.parent_rotation_offset;

            let t = (x, y, rot);
            transforms[idx] = Some(t);
            t
        } else {
            let t = (part.location.0, part.location.1, part.rotation);
            transforms[idx] = Some(t);
            t
        }
    }

    for i in 0..frame.parts.len() {
        compute_transform(i, &frame.parts, &mut transforms);
    }

    // 5. Resolve per-part geometry
    let mut parts: Vec<PartInfo> = Vec::new();

    for (i, part) in frame.parts.iter().enumerate() {
        if part.idx < 0 || part.idx >= BODY_MAX {
            continue;
        }
        let idx = part.idx as usize;

        let (src_x, src_y, src_w, src_h, anchor_x, anchor_y) = match tex_meta {
            Some(meta) => match meta.cells.get(idx).and_then(|c| c.as_ref()) {
                Some(cell) => {
                    let (sx, sy, sw, sh) = cell.src_rect;
                    let (ox, oy) = cell.origin;
                    (sx, sy, sw, sh, ox - sx as f32, oy - sy as f32)
                }
                None => continue,
            },
            None => {
                let sx = (part.idx % fallback_cols) * 128;
                let sy = (part.idx / fallback_cols) * 128;
                (sx, sy, 128i32, 128i32, 64.0_f32, 64.0_f32)
            }
        };

        if src_x >= sheet_w || src_y >= sheet_h || src_w <= 0 || src_h <= 0 {
            continue;
        }

        let (cx, cy, rot) = transforms[i].unwrap();

        // Game natively authors facing right (face = 1). We do not negate cy here,
        // as XNA transforms are intrinsically Y-Down in the engine.
        parts.push(PartInfo {
            src_x,
            src_y,
            src_w,
            src_h,
            anchor_x,
            anchor_y,
            cx,
            cy,
            rot,
            scale_x: part.scaling.0,
            scale_y: part.scaling.1,
            flip: part.flip,
        });
    }

    if parts.is_empty() {
        eprintln!("[assemble] No drawable parts for frame");
        return None;
    }

    // 6. Bounding box (considering rotation, scale, and exact anchor)
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for p in &parts {
        let left = -p.anchor_x * p.scale_x;
        let right = (p.src_w as f32 - p.anchor_x) * p.scale_x;
        let top = -p.anchor_y * p.scale_y;
        let bottom = (p.src_h as f32 - p.anchor_y) * p.scale_y;

        let corners = [(left, top), (right, top), (right, bottom), (left, bottom)];

        for (dx, dy) in corners {
            let rx = dx * p.rot.cos() - dy * p.rot.sin();
            let ry = dx * p.rot.sin() + dy * p.rot.cos();
            min_x = min_x.min(p.cx + rx);
            min_y = min_y.min(p.cy + ry);
            max_x = max_x.max(p.cx + rx);
            max_y = max_y.max(p.cy + ry);
        }
    }

    let canvas_w = (max_x - min_x).ceil() as u32;
    let canvas_h = (max_y - min_y).ceil() as u32;
    if canvas_w == 0 || canvas_h == 0 {
        eprintln!("[assemble] Zero-size canvas for frame");
        return None;
    }

    // 7. Composite (Inverse Matrix Painter's Algorithm)
    let mut canvas = RgbaImage::new(canvas_w, canvas_h);

    for p in &parts {
        if p.scale_x.abs() < 0.001 || p.scale_y.abs() < 0.001 {
            continue;
        }

        let left = -p.anchor_x * p.scale_x;
        let right = (p.src_w as f32 - p.anchor_x) * p.scale_x;
        let top = -p.anchor_y * p.scale_y;
        let bottom = (p.src_h as f32 - p.anchor_y) * p.scale_y;

        let corners = [(left, top), (right, top), (right, bottom), (left, bottom)];

        let mut pmin_x = f32::MAX;
        let mut pmin_y = f32::MAX;
        let mut pmax_x = f32::MIN;
        let mut pmax_y = f32::MIN;

        for (dx, dy) in corners {
            let rx = dx * p.rot.cos() - dy * p.rot.sin();
            let ry = dx * p.rot.sin() + dy * p.rot.cos();
            pmin_x = pmin_x.min(p.cx + rx);
            pmin_y = pmin_y.min(p.cy + ry);
            pmax_x = pmax_x.max(p.cx + rx);
            pmax_y = pmax_y.max(p.cy + ry);
        }

        let start_x = (pmin_x - min_x).floor().max(0.0) as u32;
        let start_y = (pmin_y - min_y).floor().max(0.0) as u32;
        let end_x = (pmax_x - min_x).ceil().min(canvas_w as f32) as u32;
        let end_y = (pmax_y - min_y).ceil().min(canvas_h as f32) as u32;

        let inv_rot = -p.rot;

        for dy in start_y..end_y {
            for dx in start_x..end_x {
                let px = dx as f32 + min_x + 0.5;
                let py = dy as f32 + min_y + 0.5;

                let vx = px - p.cx;
                let vy = py - p.cy;

                // Un-rotate
                let rx = vx * inv_rot.cos() - vy * inv_rot.sin();
                let ry = vx * inv_rot.sin() + vy * inv_rot.cos();

                // Un-scale
                let mut sx = rx / p.scale_x;
                let sy = ry / p.scale_y;

                // Un-flip
                if p.flip != 0 {
                    sx = -sx;
                }

                if sx >= -p.anchor_x
                    && sx < (p.src_w as f32 - p.anchor_x)
                    && sy >= -p.anchor_y
                    && sy < (p.src_h as f32 - p.anchor_y)
                {
                    let tx = (sx + p.anchor_x).floor() as i32;
                    let ty = (sy + p.anchor_y).floor() as i32;

                    let src_px = (p.src_x + tx) as u32;
                    let src_py = (p.src_y + ty) as u32;

                    if src_px < sheet.width() && src_py < sheet.height() {
                        let pixel = sheet.get_pixel(src_px, src_py);
                        if pixel[3] > 0 {
                            let alpha = pixel[3] as f32 / 255.0;
                            let mut bg = *canvas.get_pixel(dx, dy);

                            let bg_a = bg[3] as f32 / 255.0;
                            let out_a = alpha + bg_a * (1.0 - alpha);
                            if out_a > 0.0 {
                                bg[0] = ((pixel[0] as f32 * alpha
                                    + bg[0] as f32 * bg_a * (1.0 - alpha))
                                    / out_a) as u8;
                                bg[1] = ((pixel[1] as f32 * alpha
                                    + bg[1] as f32 * bg_a * (1.0 - alpha))
                                    / out_a) as u8;
                                bg[2] = ((pixel[2] as f32 * alpha
                                    + bg[2] as f32 * bg_a * (1.0 - alpha))
                                    / out_a) as u8;
                                bg[3] = (out_a * 255.0) as u8;
                            }
                            canvas.put_pixel(dx, dy, bg);
                        }
                    }
                }
            }
        }
    }

    // Character-space origin (0,0) maps to canvas pixel (-min_x, -min_y).
    Some((canvas, (-min_x, -min_y)))
}
