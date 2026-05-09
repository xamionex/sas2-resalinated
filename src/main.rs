mod app;
mod config;
mod preset;
pub mod tabs;
pub mod atlas;

use crate::app::ResalinatedApp;
#[cfg(not(debug_assertions))]
use hide_console::hide_console;

/// Detects if the hardware is a Steam Deck (LCD or OLED) and forces
/// winit to ignore X11 physical dimensions to prevent massive DPI scaling.
fn apply_steam_deck_dpi_workaround() {
    #[cfg(target_os = "linux")]
    unsafe {
        use std::fs;
        if let Ok(vendor) = fs::read_to_string("/sys/devices/virtual/dmi/id/board_vendor") {
            if vendor.trim() == "Valve" {
                if let Ok(board) = fs::read_to_string("/sys/devices/virtual/dmi/id/board_name") {
                    let board_name = board.trim();
                    // "Jupiter" = LCD Deck, "Galileo" = OLED Deck
                    if board_name == "Jupiter" || board_name == "Galileo" {
                        std::env::set_var("WINIT_X11_SCALE_FACTOR", "1");
                    }
                }
            }
        }
    }
}

fn main() -> eframe::Result<()> {
    #[cfg(not(debug_assertions))]
    hide_console();

    apply_steam_deck_dpi_workaround();

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--hide-lootdef-logging") {
        sas2_parser::set_loot_logging_enabled(false);
    }
    if args.iter().any(|a| a == "--hide-monster-logging") {
        sas2_parser::set_monster_logging_enabled(false);
    }

    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "SaS2 Resalinated",
        options,
        Box::new(|_cc| Ok(Box::new(ResalinatedApp::default()))),
    )
}
