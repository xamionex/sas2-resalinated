use std::path::{Path, PathBuf};

/// Asset overrides (texture PNGs, master.zcm, char-def .zsx, custom icons) are edited in a single
/// "working" folder, then snapshotted into a preset on Save and merged into the live game config
/// only on Apply. This mirrors how loot/monster data already flows (working -> preset -> config).
///
/// Layout (identical under the working folder, each preset's `assets/` folder, and the live config
/// folder so copies/merges are a straight mirror):
///   textures/<name>.png            texture pixel overrides
///   icons/<index>.png              custom item icons
///   Content/gfx/master.zcm         sprite-cell metadata
///   Character/data/<name>.zsx      character definitions
pub const TEXTURES_DIR: &str = "textures";
pub const ICONS_DIR: &str = "icons";
pub const MASTER_REL: &str = "Content/gfx/master.zcm";
pub const CHARDATA_DIR: &str = "Character/data";
/// Composited custom-icon atlas served to the game for `gfx/items` extra rows.
pub const CUSTOM_ITEMS_REL: &str = "textures/custom_items.png";

/// The single working-assets folder the editors read and write.
pub fn working_root() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("sas2-resalinated")
        .join("working_assets")
}

/// The live config folder the loader reads from in the game install.
pub fn config_root(game_path: &Path) -> PathBuf {
    game_path.join("BepInEx/config/amione.SaS2Resalter")
}

/// The `assets/` subfolder inside a preset folder.
pub fn preset_assets_root(preset_dir: &Path) -> PathBuf {
    preset_dir.join("assets")
}

/// Recursively copy a directory tree (creating `dst`). No-op if `src` doesn't exist.
pub fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !src.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            if let Some(parent) = to.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Remove a directory tree if present (ignores absence).
pub fn remove_dir_if_exists(dir: &Path) {
    if dir.exists() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// List `<stem>` for every `*.png` directly inside `dir`.
pub fn png_stems(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("png") {
                if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                    out.push(stem.to_string());
                }
            }
        }
    }
    out
}
