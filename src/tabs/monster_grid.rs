//! The monster/item grid used by the Monsters tab and the Hazeburnt Spawns tab.
//!
//! Tiles are sprite buttons (assembled from the monster's char def and texture) with the display
//! name wrapped one word per line underneath, plus an optional short note line. Both tabs share
//! the same metrics so the grids line up the same way.

use eframe::egui;
use egui::Ui;

/// Layout metrics of one grid: icon size, fonts and the paddings the tiles consume.
pub struct GridMetrics {
    pub icon_size: f32,
    pub font_size: f32,
    pub category_font_size: f32,
    pub label_h: f32,
    pub spacing_x: f32,
    pub spacing_y: f32,
    pub pad_x: f32,
    pub pad_y: f32,
}

impl GridMetrics {
    pub fn new(
        ui: &Ui,
        icon_size: f32,
        font_size: f32,
        category_font_size: f32,
        spacing_x: f32,
        spacing_y: f32,
    ) -> Self {
        Self {
            icon_size,
            font_size,
            category_font_size,
            label_h: ui.fonts_mut(|f| f.row_height(&egui::FontId::proportional(font_size))),
            spacing_x,
            spacing_y,
            pad_x: 2.0 * ui.spacing().button_padding.x,
            pad_y: 2.0 * ui.spacing().button_padding.y,
        }
    }

    /// Size of one tile. `extra_lines` counts the note lines under the name.
    pub fn tile_size(
        &self,
        display_name: &str,
        has_icon: bool,
        extra_lines: usize,
    ) -> egui::Vec2 {
        let word_count = display_name.split_whitespace().count();
        // Image buttons are icon_size + button frame margins wide; placeholders are icon_size.
        let width = if has_icon {
            self.icon_size + self.pad_x
        } else {
            self.icon_size
        };
        let height = if has_icon {
            self.icon_size + self.pad_y
        } else {
            self.icon_size
        } + (word_count + extra_lines) as f32 * (self.label_h + self.spacing_y);
        egui::vec2(width, height)
    }
}

/// Name label: one word per line, centered, tinted when selected.
pub fn monster_label(ui: &mut Ui, title: &str, font_size: f32, selected: bool) {
    let color = if selected {
        egui::Color32::LIGHT_GREEN
    } else {
        ui.visuals().text_color()
    };
    for word in title.split_whitespace() {
        ui.add(
            egui::Label::new(egui::RichText::new(word).size(font_size).color(color))
                .wrap_mode(egui::TextWrapMode::Truncate)
                .halign(egui::Align::Center)
                .show_tooltip_when_elided(false),
        );
    }
}

/// Draws one tile: sprite button (or a placeholder while the texture loads), the wrapped name and
/// an optional one-line note. Returns the button response so the caller can wire selection or
/// toggling; call it inside a `ui.vertical` so the labels stack under the sprite.
pub fn monster_tile(
    ui: &mut Ui,
    texture: Option<&egui::TextureHandle>,
    display_name: &str,
    metrics: &GridMetrics,
    selected: bool,
    note: Option<(&str, egui::Color32)>,
) -> egui::Response {
    let response = if let Some(texture) = texture {
        ui.add(egui::Button::image(
            egui::Image::from_texture(&texture.clone())
                .fit_to_exact_size(egui::vec2(metrics.icon_size, metrics.icon_size)),
        ))
    } else {
        // Placeholder while loading.
        ui.allocate_response(
            egui::vec2(metrics.icon_size, metrics.icon_size),
            egui::Sense::click(),
        )
    };
    ui.set_max_width(response.rect.width());
    monster_label(ui, display_name, metrics.font_size, selected);
    if let Some((text, color)) = note {
        ui.add(
            egui::Label::new(
                egui::RichText::new(text)
                    .size((metrics.font_size - 1.0).max(6.0))
                    .color(color),
            )
            .wrap_mode(egui::TextWrapMode::Truncate)
            .show_tooltip_when_elided(true),
        );
    }
    response
}
