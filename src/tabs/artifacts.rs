use crate::app::ResalinatedApp;
use crate::artifact_boost::{ArtifactBoostRange, artifact_field_count, artifact_field_info};
use crate::tabs::utils::CHANGED_COLOR;
use eframe::egui;
use egui::Ui;

/// Full editor for artifact boost configs.
///
/// Equipped artifacts (talisman subtypes Attack/Defense/Utility) contribute 35 percentage values used by the game's stat formulas.
/// Each field rolls a value between Min and Max when the artifact is obtained, ticking Static uses the Static Boost value instead.
/// Values are percentages (5 = 5%).
pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    ui.add(
        egui::Label::new(
            "Each artifact field rolls a value between Min and Max when the artifact is obtained. \
            Tick Static to always use the Static Boost value instead.",
        )
        .wrap(),
    );

    let mut resets: Vec<i32> = Vec::new();
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            egui::Grid::new("artifact_boosts")
                .num_columns(5)
                .spacing([12.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Field").strong());
                    ui.label(egui::RichText::new("Min").strong());
                    ui.label(egui::RichText::new("Max").strong());
                    ui.label(egui::RichText::new("Static").strong());
                    ui.label(egui::RichText::new("Static Boost").strong());
                    ui.end_row();

                    for i in 0..artifact_field_count() {
                        let (field_id, name, _, _, main) = artifact_field_info(i);
                        let mut entry = app
                            .artifact_boosts
                            .get(&field_id)
                            .cloned()
                            .unwrap_or_else(|| ArtifactBoostRange::vanilla(field_id));
                        let changed = entry.is_modified(field_id);

                        let label = if main {
                            format!("[{}] {} (main)", field_id, name)
                        } else {
                            format!("[{}] {}", field_id, name)
                        };
                        if changed {
                            ui.colored_label(CHANGED_COLOR, &label);
                        } else {
                            ui.label(&label);
                        }

                        ui.add(
                            egui::DragValue::new(&mut entry.min)
                                .speed(0.25)
                                .range(0.0..=1000.0)
                                .suffix("%"),
                        );
                        ui.add(
                            egui::DragValue::new(&mut entry.max)
                                .speed(0.25)
                                .range(0.0..=1000.0)
                                .suffix("%"),
                        );
                        ui.checkbox(&mut entry.static_boost, "");
                        ui.add(
                            egui::DragValue::new(&mut entry.static_value)
                                .speed(0.25)
                                .range(0.0..=1000.0)
                                .suffix("%"),
                        );

                        if changed && ui.button("↺").on_hover_text("Reset to vanilla").clicked() {
                            resets.push(field_id);
                        } else if !changed {
                            ui.label("");
                        }
                        ui.end_row();

                        if entry.is_modified(field_id) {
                            app.artifact_boosts.insert(field_id, entry);
                        } else {
                            app.artifact_boosts.remove(&field_id);
                        }
                    }
                });
        });

    for field_id in resets {
        app.artifact_boosts.remove(&field_id);
    }
}
