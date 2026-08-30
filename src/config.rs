use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize)]
pub struct ResalinatedConfig {
    #[serde(default)]
    pub game_path: Option<PathBuf>,

    /// Right panel width in the Items tab (editing area).
    #[serde(default)]
    pub items_details_panel_width: f32,

    #[serde(default)]
    pub monsters_details_panel_width: f32,

    /// Manager tab: left panel (available presets) width.
    #[serde(default)]
    pub manager_left_panel_width: f32,

    #[serde(default = "default_item_icon_size")]
    pub item_icon_size: f32,

    /// Font size of item/monster names in the item grids.
    #[serde(default = "default_grid_font_size")]
    pub grid_font_size: f32,

    /// Font size of the editor sidebars (item/monster/shop detail panels).
    #[serde(default = "default_sidebar_font_size")]
    pub sidebar_font_size: f32,

    /// Font size of the top tab bar.
    #[serde(default = "default_tabs_font_size")]
    pub tabs_font_size: f32,

    /// Font size of the grid category headers (e.g. "Weapon - Greatsword").
    #[serde(default = "default_category_font_size")]
    pub category_font_size: f32,

    #[serde(default = "default_drag_sensitivity")]
    pub drag_value_sensitivity: f32,

    #[serde(default)]
    pub dummy_drag_value: f32,

    /// External image editor command used by the Textures tab's "Open in editor" action.
    /// When empty, the OS default handler for the PNG is used. The PNG path is passed as the
    /// final argument (e.g. "gimp", "C:/Program Files/Aseprite/Aseprite.exe").
    #[serde(default)]
    pub external_image_editor: String,

    /// When true, new items (and items whose type changes) get the field set of their type,
    /// copied from a vanilla item of that type. Preserves values for fields that already exist.
    #[serde(default = "default_true")]
    pub auto_type_fields: bool,

    /// When true, saving a preset over an existing folder skips the overwrite
    /// confirmation dialog and always overwrites.
    #[serde(default)]
    pub ignore_overwrite_warning: bool,

    /// Remember and restore the window position on startup.
    #[serde(default = "default_true")]
    pub save_window_position: bool,

    /// Remember and restore the window state (maximized) on startup.
    #[serde(default = "default_true")]
    pub save_window_state: bool,

    /// Last window position (outer position of the root viewport).
    #[serde(default)]
    pub window_pos: Option<[f32; 2]>,

    /// Last window inner size.
    #[serde(default)]
    pub window_size: Option<[f32; 2]>,

    /// Last window maximized state.
    #[serde(default)]
    pub window_maximized: bool,
}

pub fn default_true() -> bool {
    true
}

impl Default for ResalinatedConfig {
    fn default() -> Self {
        Self {
            game_path: None,
            items_details_panel_width: 0.0,
            manager_left_panel_width: 0.0,
            monsters_details_panel_width: 0.0,
            item_icon_size: default_item_icon_size(),
            grid_font_size: default_grid_font_size(),
            sidebar_font_size: default_sidebar_font_size(),
            tabs_font_size: default_tabs_font_size(),
            category_font_size: default_category_font_size(),
            drag_value_sensitivity: default_drag_sensitivity(),
            dummy_drag_value: 0.0,
            external_image_editor: String::new(),
            auto_type_fields: true,
            ignore_overwrite_warning: false,
            save_window_position: true,
            save_window_state: true,
            window_pos: None,
            window_size: None,
            window_maximized: false,
        }
    }
}

pub fn default_item_icon_size() -> f32 {
    52.0
}
pub fn default_grid_font_size() -> f32 {
    12.0
}
pub fn default_sidebar_font_size() -> f32 {
    14.0
}
pub fn default_tabs_font_size() -> f32 {
    14.0
}
pub fn default_category_font_size() -> f32 {
    13.0
}
pub fn default_drag_sensitivity() -> f32 {
    0.025
}

impl ResalinatedConfig {
    pub fn load() -> Self {
        let config_path = Self::config_path();
        if let Ok(data) = fs::read_to_string(&config_path) {
            if let Ok(cfg) = serde_json::from_str(&data) {
                return cfg;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        let config_path = Self::config_path();
        if let Some(parent) = config_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = fs::write(config_path, json);
        }
    }

    fn config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("sas2-resalinated")
            .join("config.json")
    }
}
