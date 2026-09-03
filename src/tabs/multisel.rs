use eframe::egui;
use egui::{PointerButton, Rect, Ui};
use std::collections::HashSet;

/// How item upgrade levels are shown on grid icons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum UpgradeStyle {
    /// No upgrade indicator.
    #[default]
    Off,
    /// Arabic digits (0-10).
    Digits,
    /// Roman numerals (I, II, ... X).
    Roman,
}

/// Format an upgrade level per the given style ("" when off).
pub fn upgrade_label(style: UpgradeStyle, level: i32) -> String {
    match style {
        UpgradeStyle::Off => String::new(),
        UpgradeStyle::Digits => level.to_string(),
        UpgradeStyle::Roman => {
            const ROMAN: [&str; 11] = [
                "0", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X",
            ];
            ROMAN
                .get(level.clamp(0, 10) as usize)
                .copied()
                .unwrap_or("")
                .to_string()
        }
    }
}

/// Paint the upgrade level badge in the bottom-right corner of the icon rect.
pub fn paint_upgrade_badge(ui: &Ui, rect: Rect, style: UpgradeStyle, level: i32) {
    if style == UpgradeStyle::Off || level <= 0 {
        return;
    }
    let text = upgrade_label(style, level);
    let font_size = ui
        .style()
        .text_styles
        .get(&egui::TextStyle::Body)
        .map(|f| f.size)
        .unwrap_or(13.0)
        * 0.9;
    let font = egui::FontId::proportional(font_size);
    let galley = ui
        .painter()
        .layout_no_wrap(text, font, egui::Color32::WHITE);
    let pad = 2.0;
    let badge_size = galley.size() + egui::vec2(pad * 2.0, pad);
    let badge_rect = Rect::from_min_max(
        egui::pos2(rect.max.x - badge_size.x, rect.max.y - badge_size.y),
        rect.max,
    );
    let painter = ui.painter();
    painter.rect_filled(badge_rect, 3.0, egui::Color32::from_black_alpha(180));
    painter.rect_stroke(
        badge_rect,
        3.0,
        egui::Stroke::new(1.0_f32, egui::Color32::from_gray(120)),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        egui::pos2(badge_rect.min.x + pad, badge_rect.min.y + pad * 0.5),
        galley,
        egui::Color32::WHITE,
    );
}

/// Movement beyond this distance turns a press into a drag (pan or selection box).
pub const CLICK_DIST: f32 = 6.0;
/// Boxes smaller than this are treated as a plain click.
pub const MIN_BOX_SIZE: f32 = 4.0;

/// Per-grid multi-select interaction state: left-click select, ctrl+click toggle, shift+click range select (shift+ctrl+click adds the range), and a shift+drag selection box.
/// `K` is the item key.
///
/// All rects and positions are in global (screen) points, which is egui's native coordinate space for both widget rects and pointer positions.
pub struct GridSel<K> {
    /// Key of the last clicked item (range-select anchor).
    pub anchor: Option<K>,
    /// Keys in display order, rebuilt every frame.
    pub display_order: Vec<K>,
    /// Screen-space press position while the primary button is held.
    press_pos: Option<egui::Pos2>,
    /// Modifier state captured when the button was pressed.
    press_shift: bool,
    press_ctrl: bool,
    /// True once a shift+drag turned into a selection box.
    pub box_active: bool,
    /// Current selection box in screen space.
    box_rect: Option<Rect>,
    /// Keys hit by the box (from the previous frame, kept for live highlight).
    pub last_target: HashSet<K>,
    /// Visible cells this frame: (screen rect, key).
    cells: Vec<(Rect, K)>,
    /// Optional custom range resolver for shift+click (e.g. path-following on a positioned grid). `(anchor, target)` -> keys to select.
    pub range_fn: Option<Box<dyn Fn(&K, &K) -> Vec<K>>>,
}

impl<K> Default for GridSel<K> {
    fn default() -> Self {
        Self {
            anchor: None,
            display_order: Vec::new(),
            press_pos: None,
            press_shift: false,
            press_ctrl: false,
            box_active: false,
            box_rect: None,
            last_target: HashSet::new(),
            cells: Vec::new(),
            range_fn: None,
        }
    }
}

impl<K: Clone + Eq + std::hash::Hash> GridSel<K> {
    /// Call once per frame before laying out the grid. Tracks the primary-button gesture.
    pub fn begin(&mut self, ui: &Ui) {
        let (down, pressed, released, latest, shift, ctrl) = ui.input(|i| {
            (
                i.pointer.button_down(PointerButton::Primary),
                i.pointer.button_pressed(PointerButton::Primary),
                i.pointer.button_released(PointerButton::Primary),
                i.pointer.latest_pos(),
                i.modifiers.shift,
                i.modifiers.ctrl,
            )
        });
        self.display_order.clear();
        self.cells.clear();

        if pressed {
            // Only start the gesture when the press lands inside the grid area.
            if latest.is_some_and(|p| ui.clip_rect().contains(p)) {
                self.press_pos = latest;
                self.press_shift = shift;
                self.press_ctrl = ctrl;
                self.box_active = false;
                self.box_rect = None;
                self.last_target.clear();
            }
        }
        if down {
            // Shift+drag draws the selection box; without shift the drag pans the view.
            if !self.box_active
                && self.press_shift
                && let (Some(p), Some(l)) = (self.press_pos, latest)
                && (l - p).length() > CLICK_DIST
            {
                self.box_active = true;
            }
            if let (Some(p), Some(l)) = (self.press_pos, latest) {
                self.box_rect = Some(Rect::from_two_pos(p, l));
            }
        }
        if released && self.box_active {
            // Final box rect for the release frame.
            if let (Some(p), Some(l)) = (self.press_pos, latest) {
                self.box_rect = Some(Rect::from_two_pos(p, l));
            }
        }
        if !down && !released {
            // Stale state (e.g. the button was released while on another tab).
            self.box_active = false;
            self.box_rect = None;
            self.press_pos = None;
            self.last_target.clear();
        }
    }

    /// Register one visible cell during layout.
    pub fn cell(&mut self, rect: Rect, key: K) {
        self.cells.push((rect, key));
    }

    /// The current selection box, or None while it is still a point.
    pub fn box_rect(&self) -> Option<Rect> {
        self.box_rect
            .filter(|r| r.width() >= MIN_BOX_SIZE || r.height() >= MIN_BOX_SIZE)
    }

    /// Is `key` inside the current selection box (live highlight during a drag)?
    pub fn is_box_hit(&self, key: &K) -> bool {
        self.box_active && self.last_target.contains(key)
    }

    /// Recompute the box target from the cells registered this frame. Call after the grid.
    pub fn update_target(&mut self) {
        self.last_target.clear();
        if let Some(box_rect) = self.box_rect() {
            for (r, k) in &self.cells {
                if r.intersects(box_rect) {
                    self.last_target.insert(k.clone());
                }
            }
        }
    }

    /// Paint the selection box overlay. Call inside the scroll content UI so the box is clipped to the grid.
    pub fn paint(&self, ui: &Ui) {
        if !self.box_active {
            return;
        }
        if let Some(rect) = self.box_rect() {
            let painter = ui.painter();
            painter.rect_filled(
                rect,
                2.0,
                egui::Color32::from_rgba_unmultiplied(100, 160, 255, 40),
            );
            painter.rect_stroke(
                rect,
                2.0,
                egui::Stroke::new(
                    1.5_f32,
                    egui::Color32::from_rgba_unmultiplied(100, 160, 255, 200),
                ),
                egui::StrokeKind::Inside,
            );
        }
    }

    /// Call once per frame after the grid.
    /// Applies the click/range/toggle/box on primary-button release and clears the gesture.
    pub fn end(&mut self, ui: &Ui, multi: &mut HashSet<K>, single: &mut Option<K>) {
        let (released, latest) = ui.input(|i| {
            (
                i.pointer.button_released(PointerButton::Primary),
                i.pointer.latest_pos(),
            )
        });
        if released && self.press_pos.is_some() {
            let shift = self.press_shift;
            let ctrl = self.press_ctrl;
            if self.box_active {
                let target = if self
                    .box_rect
                    .is_none_or(|r| r.width().max(r.height()) < MIN_BOX_SIZE)
                {
                    latest
                        .and_then(|p| {
                            self.cells
                                .iter()
                                .find(|(r, _)| r.contains(p))
                                .map(|(_, k)| k.clone())
                        })
                        .into_iter()
                        .collect::<HashSet<_>>()
                } else {
                    std::mem::take(&mut self.last_target)
                };
                *single = target.iter().next().cloned();
                self.anchor = target.iter().next().cloned();
                if shift {
                    if ctrl {
                        // Shift+Ctrl+box: add the covered items to the selection.
                        for k in target {
                            multi.insert(k);
                        }
                    } else {
                        // Shift+box: replace the selection with the covered items.
                        *multi = target;
                    }
                } else {
                    *multi = target;
                }
            } else {
                // A click: press and release within the click distance, on a cell.
                let moved = match (self.press_pos, latest) {
                    (Some(p), Some(l)) => (l - p).length() > CLICK_DIST,
                    _ => true,
                };
                if !moved {
                    let hit = latest.and_then(|p| {
                        self.cells
                            .iter()
                            .find(|(r, _)| r.contains(p))
                            .map(|(_, k)| k.clone())
                    });
                    if let Some(k) = hit {
                        let clicked_idx = self.display_order.iter().position(|x| x == &k);
                        if shift {
                            // Custom range resolver (e.g. skill tree paths) takes priority over the linear display-order range.
                            let range: Option<Vec<K>> = self
                                .range_fn
                                .as_ref()
                                .and_then(|f| self.anchor.as_ref().map(|a| f(a, &k)));
                            let range = range.or_else(|| {
                                clicked_idx.map(|ci| {
                                    let start = self
                                        .anchor
                                        .as_ref()
                                        .and_then(|a| {
                                            self.display_order.iter().position(|x| x == a)
                                        })
                                        .unwrap_or(ci);
                                    let (lo, hi) = if start <= ci {
                                        (start, ci)
                                    } else {
                                        (ci, start)
                                    };
                                    self.display_order[lo..=hi].to_vec()
                                })
                            });
                            if let Some(range) = range {
                                if ctrl {
                                    // Shift+Ctrl+click: add the range to the selection.
                                    for x in range {
                                        multi.insert(x);
                                    }
                                } else {
                                    // Shift+click: replace the selection with the range.
                                    multi.clear();
                                    for x in range {
                                        multi.insert(x);
                                    }
                                }
                                self.anchor = Some(k.clone());
                                *single = Some(k);
                            }
                        } else if ctrl {
                            if multi.contains(&k) {
                                multi.remove(&k);
                                // The deselected item must not stay highlighted: move the single selection to the last remaining multi item (or none).
                                *single = multi.iter().next().cloned();
                            } else {
                                multi.insert(k.clone());
                                *single = Some(k.clone());
                            }
                            self.anchor = Some(k.clone());
                        } else {
                            multi.clear();
                            multi.insert(k.clone());
                            self.anchor = Some(k.clone());
                            *single = Some(k);
                        }
                    }
                }
            }
            // Gesture complete: clear the state.
            // The stale-state cleanup in `begin` handles the case where the button was released elsewhere.
            self.box_active = false;
            self.box_rect = None;
            self.press_pos = None;
            self.last_target.clear();
        }
    }

    /// Abort any in-flight gesture (e.g. when switching tabs, subtabs or shops).
    pub fn reset_gesture(&mut self) {
        self.press_pos = None;
        self.press_shift = false;
        self.press_ctrl = false;
        self.box_active = false;
        self.box_rect = None;
        self.last_target.clear();
        self.cells.clear();
    }
}

/// Scroll source for grids with a selection box: shift disables drag-panning so the box can be drawn, without shift, holding left click pans the view.
pub fn grid_scroll_source(ui: &Ui) -> egui::containers::scroll_area::ScrollSource {
    if ui.input(|i| i.modifiers.shift) {
        egui::containers::scroll_area::ScrollSource::SCROLL_BAR
            | egui::containers::scroll_area::ScrollSource::MOUSE_WHEEL
    } else {
        egui::containers::scroll_area::ScrollSource::ALL
    }
}

/// Paint a green outline around a selected cell's icon button.
pub fn paint_sel_outline(ui: &Ui, rect: Rect, selected: bool) {
    if selected {
        ui.painter().rect_stroke(
            rect,
            2.0,
            egui::Stroke::new(2.0_f32, egui::Color32::LIGHT_GREEN),
            egui::StrokeKind::Inside,
        );
    }
}

/// Shared multi-selection help text shown by the "Mouse usage help" button.
pub const MULTISEL_HELP: &str = "Multi-select:\n\
    \u{2022} Left click: select an item\n\
    \u{2022} Ctrl + click: toggle an item in/out of the selection\n\
    \u{2022} Shift + click: select the range between the last click and the item (replaces the selection)\n\
    \u{2022} Shift + Ctrl + click: add the range to the selection\n\
    \u{2022} Shift + hold and drag: draw a selection box over the items to select\n\
    \u{2022} Shift + Ctrl + hold and drag: add the boxed items to the selection\n\
    \u{2022} Hold left click and drag: move the view\n\
    \n\
    Selected items are edited together in the sidebar.";

/// Draw a "Mouse usage help" button that opens a popup with the shared help text plus any tab-specific lines.
/// Call it in the tab header row.
pub fn mouse_help_button(ui: &mut Ui, extra_lines: &[&str]) {
    let mut open = false;
    ui.menu_button("Mouse usage help", |ui| {
        open = true;
        ui.label(egui::RichText::new(MULTISEL_HELP).weak());
        for line in extra_lines {
            ui.separator();
            ui.label(egui::RichText::new(*line).weak());
        }
    });
    let _ = open;
}
