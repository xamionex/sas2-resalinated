pub mod animations;
pub mod artifacts;
pub mod images;
pub mod items;
pub mod manager;
pub mod monsters;
pub mod preset_info;
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
}
