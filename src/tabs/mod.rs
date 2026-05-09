pub mod preset_info;
pub mod items;
pub mod manager;
pub mod monsters;

#[derive(PartialEq)]
pub enum Tab {
    PresetInfo,
    Items,
    Manager,
    Monsters,
}