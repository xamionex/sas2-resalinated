use sas2_parser::loot_catalog::LootCatalog;
use sas2_parser::monster_catalog::MonsterCatalog;
use sas2_parser::skilltree::SkillTreeCatalog;
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
