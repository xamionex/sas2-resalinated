use eframe::epaint::Color32;
use egui::Ui;

pub(crate) const CHANGED_COLOR: Color32 = Color32::from_rgb(200, 160, 0);

/// Helper to draw a labeled horizontal row with change-tracking color.
pub(crate) fn field_row(ui: &mut Ui, label: &str, changed: bool, content: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        if changed {
            ui.colored_label(CHANGED_COLOR, label);
        } else {
            ui.label(label);
        }
        content(ui);
    });
}
