use crate::app::ResalinatedApp;
use crate::charm_boost::{CharmBoostRange, CharmBoostUnit, charm_boost_unit};
use crate::tabs::utils::CHANGED_COLOR;
use eframe::egui;
use egui::Ui;
use sas2_parser::loot_names;

/// Number of charm (talisman) boost flags in the game (0..=54).
const CHARM_FLAG_COUNT: i32 = 55;

fn unit_suffix(unit: CharmBoostUnit) -> String {
    match unit {
        CharmBoostUnit::Percent => "%".to_string(),
        CharmBoostUnit::Flat => String::new(),
    }
}

/// Full editor for talisman boost configs.
///
/// Each boost (flag 0..=54, e.g. "Phys Def", "Item Find", "Max HP Boost") rolls a value between Min and Max when the talisman is given to the player.
/// When the Static checkbox is set, the Static Boost value is used instead of the roll.
/// Values are the actual in-game magnitudes (10 = 10%), pre-filled with the vanilla value of each boost.
pub fn show(app: &mut ResalinatedApp, ui: &mut Ui) {
    ui.add(
        egui::Label::new(
            "Each boost rolls a value between Min and Max when the talisman is given. \
            Tick Static to always use the Static Boost value instead.",
        )
        .wrap(),
    );

    let mut resets: Vec<i32> = Vec::new();
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            egui::Grid::new("charm_boosts")
                .num_columns(5)
                .spacing([12.0, 4.0])
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Boost").strong());
                    ui.label(egui::RichText::new("Min").strong());
                    ui.label(egui::RichText::new("Max").strong());
                    ui.label(egui::RichText::new("Static").strong());
                    ui.label(egui::RichText::new("Static Boost").strong());
                    ui.end_row();

                    for flag in 0..CHARM_FLAG_COUNT {
                        let name = loot_names::get_flag_name(6, flag);
                        let unit = charm_boost_unit(flag);
                        let suffix = unit_suffix(unit);

                        let mut entry = app
                            .charm_boosts
                            .get(&flag)
                            .cloned()
                            .unwrap_or_else(|| CharmBoostRange::vanilla(flag));
                        let changed = entry.is_modified(flag);

                        if changed {
                            ui.colored_label(CHANGED_COLOR, format!("[{}] {}", flag, name));
                        } else {
                            ui.label(format!("[{}] {}", flag, name));
                        }

                        ui.add(
                            egui::DragValue::new(&mut entry.min)
                                .speed(0.5)
                                .range(0.0..=1000.0)
                                .suffix(&suffix),
                        );
                        ui.add(
                            egui::DragValue::new(&mut entry.max)
                                .speed(0.5)
                                .range(0.0..=1000.0)
                                .suffix(&suffix),
                        );
                        ui.checkbox(&mut entry.static_boost, "");
                        ui.add(
                            egui::DragValue::new(&mut entry.static_value)
                                .speed(0.5)
                                .range(0.0..=1000.0)
                                .suffix(&suffix),
                        );

                        if changed && ui.button("↺").on_hover_text("Reset to vanilla").clicked() {
                            resets.push(flag);
                        } else if !changed {
                            ui.label("");
                        }
                        ui.end_row();

                        // Persist: only keep entries that differ from vanilla.
                        if entry.is_modified(flag) {
                            app.charm_boosts.insert(flag, entry);
                        } else {
                            app.charm_boosts.remove(&flag);
                        }
                    }
                });
        });

    for flag in resets {
        app.charm_boosts.remove(&flag);
    }
}
