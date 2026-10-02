pub mod animations;
pub mod artifacts;
pub mod hazeburnt_spawns;
pub mod images;
pub mod items;
pub mod manager;
pub mod monster_grid;
pub mod monsters;
pub mod multisel;
pub mod preset_info;
pub mod projectiles;
pub mod shop;
pub mod talismans;
pub mod textures;
pub mod utils;

#[derive(PartialEq)]
pub enum Tab {
    PresetInfo,
    Items,
    Manager,
    Monsters,
    Textures,
    Animations,
    Images,
    Shop,
    Talismans,
    Artifacts,
    Projectiles,
    HazeburntSpawns,
}

impl Tab {
    /// Short name used by diagnostics (frame-time log).
    pub fn name(&self) -> &'static str {
        match self {
            Tab::PresetInfo => "PresetInfo",
            Tab::Items => "Items",
            Tab::Manager => "Manager",
            Tab::Monsters => "Monsters",
            Tab::Textures => "Textures",
            Tab::Animations => "Animations",
            Tab::Images => "Images",
            Tab::Shop => "Shop",
            Tab::Talismans => "Talismans",
            Tab::Artifacts => "Artifacts",
            Tab::Projectiles => "Projectiles",
            Tab::HazeburntSpawns => "HazeburntSpawns",
        }
    }
}
