use sas2_parser::dialog::DialogCatalog;
use sas2_parser::loot_catalog::LootCatalog;
use sas2_parser::map::MapData;
use sas2_parser::monster_catalog::MonsterCatalog;
use sas2_parser::skilltree::SkillTreeCatalog;
use sas2_parser::subflags::SubFlagDefCatalog;
use sas2_parser::xtexture::MasterTextures;
use std::fs;
use std::path::Path;

pub fn load_loot_catalog(game_path: &Path) -> Result<LootCatalog, String> {
    let loot_path = game_path.join("Loot").join("data").join("loot.zls");
    if !loot_path.exists() {
        return Err(format!("loot.zls not found in: {}", loot_path.display()));
    }
    let data = fs::read(&loot_path).map_err(|e| e.to_string())?;
    LootCatalog::load_from_bytes(&data).map_err(|e| e.to_string())
}

pub fn load_dialog_catalog(game_path: &Path) -> Result<DialogCatalog, String> {
    let dialog_path = game_path.join("Dialog").join("data").join("dialog.zdx");
    if !dialog_path.exists() {
        return Err(format!(
            "dialog.zdx not found in: {}",
            dialog_path.display()
        ));
    }
    let data = fs::read(&dialog_path).map_err(|e| e.to_string())?;
    DialogCatalog::load_from_bytes(&data).map_err(|e| e.to_string())
}

/// Load the texture catalog (master.zcm + flagdefs.zfd) needed to parse maps.
pub fn load_texture_catalog(
    game_path: &Path,
) -> Result<(MasterTextures, SubFlagDefCatalog), String> {
    let flagdefs_path = game_path.join("Content").join("gfx").join("flagdefs.zfd");
    let flag_defs = SubFlagDefCatalog::load_from_path(&flagdefs_path)
        .map_err(|e| format!("Failed to load flagdefs.zfd: {}", e))?;
    let master_path = game_path.join("Content").join("gfx").join("master.zcm");
    let master = MasterTextures::load_from_path(&master_path, &flag_defs)
        .map_err(|e| format!("Failed to load master.zcm: {}", e))?;
    Ok((master, flag_defs))
}

/// Load a .zax map file's entity layer (NPC/merchant placements).
#[allow(dead_code)]
pub fn load_map_entities(
    game_path: &Path,
    map_name: &str,
    master: &MasterTextures,
    flag_defs: &SubFlagDefCatalog,
) -> Result<MapData, String> {
    let map_path = game_path.join("Map").join("data").join(format!("{}.zax", map_name));
    if !map_path.exists() {
        return Err(format!("{}.zax not found in: {}", map_name, map_path.display()));
    }
    let data = fs::read(&map_path).map_err(|e| e.to_string())?;
    MapData::load_from_bytes(&data, master, flag_defs).map_err(|e| e.to_string())
}

pub fn load_monster_catalog(game_path: &Path) -> Result<MonsterCatalog, String> {
    let monsters_path = game_path.join("Monsters").join("data").join("monsters.zms");
    if !monsters_path.exists() {
        return Err(format!(
            "monsters.zms not found in: {}",
            monsters_path.display()
        ));
    }
    let data = fs::read(&monsters_path).map_err(|e| e.to_string())?;
    MonsterCatalog::load_from_bytes(&data).map_err(|e| e.to_string())
}

// TODO: implement skill tree modification
#[allow(dead_code)]
pub fn load_skilltree_catalog(game_path: &Path) -> Result<SkillTreeCatalog, String> {
    let skilltree_path = game_path
        .join("SkillTree")
        .join("data")
        .join("skilltree.zsx");
    if !skilltree_path.exists() {
        return Err(format!(
            "skilltree.zsx not found in: {}",
            skilltree_path.display()
        ));
    }
    SkillTreeCatalog::load_from_path(&skilltree_path)
}

#[allow(dead_code)]
pub fn load_skilltree_texture(
    game_path: &Path,
    ctx: &egui::Context,
) -> Result<egui::TextureHandle, String> {
    // Skill icons are on the main UI atlas
    let interface_xnb = game_path.join("Content").join("gfx").join("interface.xnb");
    if interface_xnb.exists() {
        let img = sas2_parser::xnb_loader::load_texture_from_path(interface_xnb.to_str().unwrap())?;
        let width = img.width();
        let height = img.height();
        let pixels = img.into_vec();
        let size = [width as usize, height as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
        return Ok(ctx.load_texture("interface_atlas", color_image, Default::default()));
    }
    Err("interface.xnb not found".to_string())
}
