//! Interface panels: the dock at the top left (heads-up display, notebook and calculator), the
//! handbook beside it (its pages are drawn by `handbook`),
//! the station, the ruler's reading, and the body of the conversation with Lumen (drawn by the
//! companion, attached to Lumen's eye).
//!
//! The notebook, the handbook and Lumen are always on screen. Reduced, they show only their header
//! (Lumen: its eye); their key, or a click on the header, expands and reduces them. The handbook has
//! its own place beside the dock's column, where it opens downward to read at full height.

use crate::carry::{Carry, Item};
use crate::companion::Landmarks;
use crate::player::OrbitCamera;
use crate::ruler::Ruler;
use crate::sim::{CRAWLER_TIME, Request, Sim};
use crate::state::{Game, InputTarget, Mode, Ui};
use crate::tutor_bridge::Lumen;
use crate::waking::Waking;
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use egui::{Align2, Color32, RichText};
use physics_core::ballistics::FlightEnd;
use physics_core::content::{DemoDef, PuzzleDef};
use physics_core::expr::{evaluate, is_function};
use physics_core::notebook::source;
use physics_core::progress::Quality;
use physics_core::puzzle::{Ch1Layout, InputSource, Instance, Outcome, Playback, PuzzleKind};
use physics_core::units::{Quantity, lookup_unit, sig};
use tutor::Turn;

pub(crate) const GOOD: Color32 = Color32::from_rgb(110, 220, 130);
pub(crate) const WARN: Color32 = Color32::from_rgb(240, 190, 80);
pub(crate) const BAD: Color32 = Color32::from_rgb(240, 110, 100);
pub(crate) const DIM: Color32 = Color32::from_rgb(160, 165, 175);
/// Lumen's colour, the cyan of its eye, for its words in the panels.
pub(crate) const LUMEN: Color32 = Color32::from_rgb(110, 205, 240);

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Ui>()
            .add_systems(Update, hotkeys.run_if(in_state(Mode::Playing)))
            .add_systems(EguiPrimaryContextPass, (style, draw.after(style).run_if(in_state(Mode::Playing))));
    }
}

/// Width of the dock's panels when expanded, and of its headers when reduced.
const DOCK_WIDTH: f32 = 460.0;
const DOCK_HEADER_WIDTH: f32 = 250.0;
/// Height kept free at the bottom of the screen for Lumen's eye, and how much more while the ruler's
/// card is up (the eye rises above it).
const DOCK_BOTTOM_RESERVE: f32 = 170.0;
const DOCK_RULER_RESERVE: f32 = 140.0;
/// And while Lumen's conversation is open above the eye.
const DOCK_LUMEN_RESERVE: f32 = 190.0;
/// The handbook's reading window: its width, and the height kept free under it for the hints.
const HANDBOOK_WIDTH: f32 = 540.0;
/// Where the handbook starts: clear of the column's widest panel, whose frame adds its margins and
/// border to the dock's width, with a gap like the one between the column's panels.
const HANDBOOK_LEFT: f32 = 10.0 + DOCK_WIDTH + 2.0 * (6.0 + 1.0) + 10.0;
const HANDBOOK_BOTTOM_RESERVE: f32 = 80.0;

fn hotkeys(
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    waking: Res<Waking>,
    mut ui: ResMut<Ui>,
    mut ruler: ResMut<Ruler>,
    mut orbit: ResMut<OrbitCamera>,
) {
    if wants.wants_any_keyboard_input() {
        return;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        ui.notebook_open = !ui.notebook_open;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        ui.handbook_open = !ui.handbook_open;
        // Opening it with a new page waiting opens it at that page.
        if ui.handbook_open {
            ui.handbook_focus = ui.unread_pages.last().cloned();
        }
    }
    if keys.just_pressed(KeyCode::KeyT) {
        ui.tutor_open = !ui.tutor_open;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        // R starts a measurement, or cancels one half taken. A finished reading is replaced.
        if ruler.active { ruler.put_away() } else { ruler.start() }
    }
    if keys.just_pressed(KeyCode::F1) {
        ui.controls_open = !ui.controls_open;
    }
    if keys.just_pressed(KeyCode::Escape) && !waking.playing() {
        // One thing at a time: the ruler, the station, any open panel, or cursor free-look.
        if ruler.in_use() {
            ruler.put_away();
        } else if ui.at_console() {
            ui.station = None;
            ui.beam_open = false;
        } else if ui.notebook_open || ui.handbook_open || ui.tutor_open || ui.controls_open {
            ui.notebook_open = false;
            ui.handbook_open = false;
            ui.tutor_open = false;
            ui.controls_open = false;
        } else {
            orbit.cursor_released = !orbit.cursor_released;
        }
    }
}

/// Record a HUD panel's screen rectangle so Lumen's world-space labels stay clear of it.
fn record_rect(ui_state: &mut Ui, rect: egui::Rect) {
    ui_state.occupied.push([rect.min.x, rect.min.y, rect.max.x, rect.max.y]);
}

/// The interface's scale, set once for the menu and the game alike.
pub fn style(mut contexts: EguiContexts, mut done: Local<bool>) -> Result {
    if !*done {
        contexts.ctx_mut()?.set_zoom_factor(1.15);
        *done = true;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut game: ResMut<Game>,
    mut ui_state: ResMut<Ui>,
    mut sim: ResMut<Sim>,
    mut ruler: ResMut<Ruler>,
    carry: Res<Carry>,
    waking: Res<Waking>,
    orbit: Res<OrbitCamera>,
    mut handbook_was_open: Local<bool>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    // Reborrow once so the panels can borrow fields separately.
    let (game, ui_state, sim, ruler) = (&mut *game, &mut *ui_state, &mut *sim, &mut *ruler);
    ui_state.occupied.clear();

    // While a wake plays behind letterbox bars, hide the HUD panels so nothing clips under the bars.
    if waking.playing() {
        return Ok(());
    }

    dock(&ctx, game, ui_state, ruler);
    hints(&ctx, game, &waking, ui_state, ruler, &carry, &orbit);
    controls_help(&ctx, ui_state);
    if let Some(puzzle) = ui_state.station {
        station_panel(&ctx, game, ui_state, sim, puzzle);
    }
    if ruler.in_use() {
        ruler_panel(&ctx, game, ui_state, ruler);
    }
    if ruler.active {
        ruler_hover_label(&ctx, ruler);
    }
    if !ui_state.at_console() {
        carry_badge(&ctx, &carry);
    }
    if orbit.free_look && carry.holding == Some(Item::Gate) {
        let centre = ctx.content_rect().center();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("aim-dot")));
        painter.circle_filled(centre, 3.0, Color32::from_rgba_unmultiplied(255, 220, 120, 220));
        painter.circle_stroke(centre, 5.5, egui::Stroke::new(1.0, Color32::from_rgba_unmultiplied(15, 20, 28, 190)));
    }
    if ui_state.handbook_open {
        *handbook_was_open = true;
    } else if *handbook_was_open {
        // New pages count as read once the handbook has been opened and reduced again.
        *handbook_was_open = false;
        ui_state.unread_pages.clear();
    }
    Ok(())
}

/// The column at the top left: where the chapter stands, then the notebook. Its place is fixed.
fn dock(ctx: &egui::Context, game: &mut Game, ui_state: &mut Ui, ruler: &Ruler) {
    let screen = ctx.content_rect();
    let reserve = DOCK_BOTTOM_RESERVE + if ruler.in_use() { DOCK_RULER_RESERVE } else { 0.0 } + if ui_state.tutor_open { DOCK_LUMEN_RESERVE } else { 0.0 };
    let floor = screen.bottom() - reserve;
    let area = egui::Area::new(egui::Id::new("dock")).anchor(Align2::LEFT_TOP, [10.0, 10.0]).movable(false).show(ctx, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        egui::Frame::window(ui.style()).show(ui, |ui| {
            ui.set_max_width(DOCK_WIDTH);
            ui.label(RichText::new(format!("Tier {} · {}", game.tier.number, game.tier.goal)).small().color(DIM));
            ui.label(RichText::new(format!("{}  {}", game.chapter.id, game.chapter.title)).strong());
            if !game.chapter.goal.is_empty() {
                ui.label(RichText::new(&game.chapter.goal).color(DIM));
            }
            let marks: String = (0..game.chapter.puzzles.len())
                .map(|i| {
                    if game.solved(i) {
                        "■ "
                    } else if game.unlocked(i) {
                        "□ "
                    } else {
                        "· "
                    }
                })
                .collect();
            match game.next_unsolved() {
                Some(next) => ui.label(format!("{marks} Next: {}", game.def(next).task)),
                None => ui.label(RichText::new(format!("{marks} {} installed. Chapter complete.", game.chapter.part)).color(GOOD)),
            };
            if let Some(problem) = &game.save_problem {
                ui.colored_label(BAD, problem);
            }
        });

        egui::Frame::window(ui.style()).show(ui, |ui| {
            ui.set_width(if ui_state.notebook_open { DOCK_WIDTH } else { DOCK_HEADER_WIDTH });
            if dock_header(ui, "Notebook and calculator", "N", ui_state.notebook_open, None) {
                ui_state.notebook_open = !ui_state.notebook_open;
            }
            if ui_state.notebook_open {
                ui.separator();
                let room = (floor - ui.cursor().top() - 20.0).max(70.0);
                // An area's contents are capped at its size the frame before; give the body its room.
                ui.allocate_ui(egui::vec2(ui.available_width(), room), |ui| notebook_body(ui, game, ui_state, room));
            }
        });
    });
    ui_state.dock_bottom = area.response.rect.bottom();
    record_rect(ui_state, area.response.rect);
    handbook_panel(ctx, game, ui_state);
}

/// The handbook, at its place beside the dock's column. Reduced, it is only its header; expanded, it
/// runs down from there to the hints.
fn handbook_panel(ctx: &egui::Context, game: &mut Game, ui_state: &mut Ui) {
    let screen = ctx.content_rect();
    let left = HANDBOOK_LEFT;
    let area = egui::Area::new(egui::Id::new("handbook")).anchor(Align2::LEFT_TOP, [left, 10.0]).movable(false).show(ctx, |ui| {
        egui::Frame::window(ui.style()).show(ui, |ui| {
            ui.set_width(if ui_state.handbook_open { HANDBOOK_WIDTH } else { DOCK_HEADER_WIDTH });
            let unread = ui_state.unread_pages.len();
            let badge = (unread > 0).then(|| format!("{unread} new"));
            if dock_header(ui, "Handbook", "H", ui_state.handbook_open, badge) {
                ui_state.handbook_open = !ui_state.handbook_open;
                if ui_state.handbook_open {
                    ui_state.handbook_focus = ui_state.unread_pages.last().cloned();
                }
            }
            if ui_state.handbook_open {
                ui.separator();
                let room = (screen.bottom() - HANDBOOK_BOTTOM_RESERVE - ui.cursor().top()).max(120.0);
                // An area's contents are capped at its size the frame before, which was only the
                // header when the handbook was just opened; give the body the room it has.
                ui.allocate_ui(egui::vec2(ui.available_width(), room), |ui| crate::handbook::body(ui, game, ui_state, room));
            }
        });
    });
    record_rect(ui_state, area.response.rect);
}

/// A dock panel's header: an arrow, the title, an optional badge, and the key that toggles it. The
/// whole row is a button. Returns whether it was clicked.
pub(crate) fn dock_header(ui: &mut egui::Ui, title: &str, key: &str, open: bool, badge: Option<String>) -> bool {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), egui::Sense::click());
    let hovered = response.hovered();
    let text = if hovered { Color32::WHITE } else { ui.visuals().strong_text_color() };
    let painter = ui.painter();
    // The arrow: right when reduced, down when expanded.
    let c = egui::pos2(rect.left() + 6.0, rect.center().y);
    let arrow = if open {
        vec![c + egui::vec2(-5.0, -3.0), c + egui::vec2(5.0, -3.0), c + egui::vec2(0.0, 4.0)]
    } else {
        vec![c + egui::vec2(-3.0, -5.0), c + egui::vec2(4.0, 0.0), c + egui::vec2(-3.0, 5.0)]
    };
    painter.add(egui::Shape::convex_polygon(arrow, text, egui::Stroke::NONE));
    let title_rect = painter.text(egui::pos2(rect.left() + 18.0, rect.center().y), Align2::LEFT_CENTER, title, egui::FontId::proportional(14.0), text);
    if let Some(badge) = badge {
        painter.text(egui::pos2(title_rect.right() + 8.0, rect.center().y), Align2::LEFT_CENTER, format!("· {badge}"), egui::FontId::proportional(13.0), GOOD);
    }
    // The key, as a small keycap at the right.
    let cap = egui::Rect::from_center_size(egui::pos2(rect.right() - 10.0, rect.center().y), egui::vec2(18.0, 18.0));
    painter.rect_stroke(cap, 3.0, egui::Stroke::new(1.0, DIM), egui::StrokeKind::Inside);
    painter.text(cap.center(), Align2::CENTER_CENTER, key, egui::FontId::monospace(12.0), DIM);
    let verb = if open { "Reduce" } else { "Expand" };
    response.on_hover_text(format!("{verb} ({key})")).clicked()
}

fn hints(ctx: &egui::Context, game: &Game, waking: &Waking, ui_state: &mut Ui, ruler: &Ruler, carry: &Carry, orbit: &OrbitCamera) {
    let prompt = if ruler.active {
        Some("Left-click two points to measure between them. R or Esc cancels.".to_string())
    } else if let Some(line) = crate::carry::hint(carry).filter(|_| !ui_state.at_console()) {
        Some(line)
    } else if ui_state.beam_open {
        Some("You're at the beam's console. Press E or Esc to leave.".to_string())
    } else if ui_state.near_beam {
        Some("Press E to use the console: The beam".to_string())
    } else {
        match (ui_state.near_station, ui_state.station) {
            (_, Some(_)) => Some("You're at the station. Press E or Esc to leave.".to_string()),
            (Some(near), None) if crate::stations::ready(game, waking, near) => Some(format!("Press E to use the station: {}", game.def(near).title)),
            (Some(near), None) if game.unlocked(near) => Some("This station is waking up.".to_string()),
            (Some(near), None) => {
                let needs: Vec<&str> =
                    game.def(near).requires.iter().filter_map(|id| game.chapter.puzzle(id)).map(|p| crate::stations::machine_name(p.kind)).collect();
                Some(format!("This station is still asleep. Finish the job at {} first.", needs.join(" and ")))
            }
            _ if orbit.cursor_released
                && !ui_state.notebook_open
                && !ui_state.handbook_open
                && !ui_state.tutor_open
                && !ui_state.controls_open
                && !waking.playing() =>
            {
                Some("Cursor free. Click the world or press Esc to look around.".to_string())
            }
            _ => None,
        }
    };
    let unread = (!ui_state.unread_pages.is_empty() && !ui_state.handbook_open).then(|| match ui_state.unread_pages.as_slice() {
        [one] => format!("New handbook page: {}. Press H to read it.", game.chapter.concept(one).map_or(one.as_str(), |c| c.title.as_str())),
        many => format!("{} new handbook pages. Press H to read them.", many.len()),
    });
    if prompt.is_none() && unread.is_none() {
        return;
    }
    if let Some(resp) = egui::Window::new("hints").title_bar(false).resizable(false).anchor(Align2::CENTER_BOTTOM, [0.0, -10.0]).show(ctx, |ui| {
        if let Some(prompt) = prompt {
            ui.vertical_centered(|ui| ui.label(RichText::new(prompt).strong().color(WARN)));
        }
        if let Some(line) = unread {
            ui.vertical_centered(|ui| ui.label(RichText::new(line).strong().color(GOOD)));
        }
    }) {
        record_rect(ui_state, resp.response.rect);
    }
}

/// Compact `?` toggle in the lower right (and `F1`) that opens a reference card of all controls.
fn controls_help(ctx: &egui::Context, ui_state: &mut Ui) {
    let x_offset = if ui_state.at_console() { -424.0 } else { -10.0 };
    let area = egui::Area::new(egui::Id::new("controls-toggle")).anchor(Align2::RIGHT_BOTTOM, [x_offset, -10.0]).movable(false).show(ctx, |ui| {
        egui::Frame::window(ui.style()).inner_margin(egui::Margin::symmetric(8, 4)).show(ui, |ui| {
            let label = if ui_state.controls_open { "Controls (F1) ×" } else { "?  F1" };
            if ui.add(egui::Button::new(RichText::new(label).small().color(DIM)).frame(false)).on_hover_text("Show or hide controls (F1)").clicked() {
                ui_state.controls_open = !ui_state.controls_open;
            }
        });
    });
    record_rect(ui_state, area.response.rect);

    if !ui_state.controls_open {
        return;
    }
    let mut open = true;
    if let Some(resp) =
        egui::Window::new("Controls").open(&mut open).collapsible(false).resizable(false).anchor(Align2::RIGHT_BOTTOM, [x_offset, -44.0]).show(ctx, |ui| {
            egui::Grid::new("controls-grid").num_columns(2).spacing([14.0, 4.0]).striped(true).show(ui, |ui| {
                for (keys, action) in [
                    ("WASD", "Move"),
                    ("Shift", "Run"),
                    ("Space", "Jump"),
                    ("Mouse / Arrows", "Look around"),
                    ("Scroll", "Zoom"),
                    ("Alt (hold) / Esc", "Free cursor"),
                    ("E", "Use or leave a station"),
                    ("F", "Pick up, put down or clamp"),
                    ("R", "Measure with the ruler"),
                    ("N", "Notebook and calculator"),
                    ("H", "Handbook"),
                    ("T", "Lumen"),
                    ("Enter", "Next Lumen line"),
                    ("F1", "Show or hide controls"),
                ] {
                    ui.label(RichText::new(keys).strong().monospace());
                    ui.label(RichText::new(action).color(DIM));
                    ui.end_row();
                }
            });
        })
    {
        record_rect(ui_state, resp.response.rect);
    }
    if !open {
        ui_state.controls_open = false;
    }
}

/// Floating world-space badge beside a carry placement ghost (drop tower notch, jack frame height,
/// or beam seat).
fn carry_badge(ctx: &egui::Context, carry: &Carry) {
    let Some((screen_px, text, is_put)) = &carry.badge else { return };
    let zoom = ctx.zoom_factor();
    let screen = ctx.content_rect();
    let pos = egui::pos2(screen_px.x / zoom, screen_px.y / zoom);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("carry-badge")));
    let accent = if *is_put { Color32::from_rgb(100, 230, 255) } else { WARN };
    let galley = painter.layout_no_wrap(text.clone(), egui::FontId::proportional(13.0), Color32::from_rgb(240, 246, 255));
    let size = galley.size() + egui::vec2(14.0, 8.0);
    let mut rect = egui::Rect::from_center_size(pos + egui::vec2(0.0, -size.y / 2.0 - 8.0), size);
    rect = rect.translate(egui::vec2(
        (screen.left() + 8.0 - rect.left()).max(0.0) - (rect.right() - screen.right() + 8.0).max(0.0),
        (screen.top() + 8.0 - rect.top()).max(0.0) - (rect.bottom() - screen.bottom() + 8.0).max(0.0),
    ));
    painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(12, 20, 32, 220));
    painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.2, accent), egui::StrokeKind::Inside);
    painter.galley(rect.min + egui::vec2(7.0, 4.0), galley, Color32::WHITE);
}

fn station_panel(ctx: &egui::Context, game: &mut Game, ui_state: &mut Ui, sim: &mut Sim, puzzle: usize) {
    let def = game.def(puzzle).clone();
    let instance = game.instance(puzzle);
    let specs = instance.inputs();
    ui_state.drafts[puzzle].resize(specs.len(), String::new());

    if let Some(resp) = egui::Window::new(format!("Station: {}", def.title))
        .id(egui::Id::new("station"))
        .collapsible(false)
        .resizable(false)
        .default_width(400.0)
        // A window with a scroll area is otherwise capped at egui's default window height.
        .default_height(ctx.content_rect().height() - 30.0)
        .anchor(Align2::RIGHT_TOP, [-10.0, 10.0])
        .show(ctx, |ui| {
            ui.set_width(400.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("You'll stay at the console while this is open.").small().color(DIM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Leave (E)").clicked() {
                        ui_state.station = None;
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(ctx.content_rect().height() - 110.0).show(ui, |ui| {
                if !def.why.is_empty() {
                    ui.label(RichText::new(format!("Lumen: {}", def.why)).italics().color(LUMEN));
                    ui.add_space(4.0);
                }
                if def.kind == PuzzleKind::Ridge {
                    ui.label(RichText::new(format!("Launcher rating: {} maximum.", Quantity::speed(game.layout.v_max).display())).strong());
                }
                if def.kind == PuzzleKind::Chase {
                    ui.label(RichText::new(format!("Haul cart rating: {} top speed.", Quantity::speed(Ch1Layout::CART_TOP_SPEED).display())).strong());
                }
                if def.kind == PuzzleKind::Channel {
                    rating_plate(ui, game);
                }
                ui.add_space(4.0);
                egui::CollapsingHeader::new(RichText::new("Brief and how to measure").strong())
                    .id_salt(("station-brief", puzzle))
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label(&def.brief);
                        ui.add_space(6.0);
                        ui.label(RichText::new("Investigate").strong());
                        ui.label(RichText::new(&def.investigate).color(DIM));
                    });

                if let Some(demo) = &def.demo {
                    ui.add_space(4.0);
                    let watched = game.save.demo_watched(&def.id);
                    let heading = if watched { "Try it first (watched)" } else { "Try it first" };
                    egui::CollapsingHeader::new(RichText::new(heading).strong())
                        .id_salt(("station-demo", puzzle))
                        .default_open(true)
                        .show(ui, |ui| {
                            experiment_section(ui, game, ui_state, sim, puzzle, demo);
                        });
                }
                ui.add_space(4.0);
                pages_row(ui, game, ui_state, &def);

                if matches!(def.kind, PuzzleKind::Chase | PuzzleKind::Gravity) {
                    ui.add_space(6.0);
                    ui.separator();
                    measure_section(ui, game, sim, puzzle);
                }

                ui.add_space(6.0);
                ui.separator();
                ui.label(RichText::new("Blueprint").strong());
                if def.kind == PuzzleKind::Rail {
                    // The cart's cargo is not a value, but the cart will not run without it.
                    let loaded = game.placed(puzzle) > 0;
                    let built = instance.built(game.placed(puzzle)).unwrap_or_default();
                    ui.horizontal(|ui| {
                        ui.label("Cargo:");
                        ui.label(RichText::new(if loaded { "loaded" } else { "not loaded" }).strong().color(if loaded { GOOD } else { WARN }));
                    });
                    let todo = if loaded { "" } else { " Leave the console and carry it on (F)." };
                    ui.label(RichText::new(format!("    {}.{todo}", capitalise(&built))).small().color(DIM));
                }
                let mut enter_commit = false;
                for (i, spec) in specs.iter().enumerate() {
                    if spec.source == InputSource::Placed {
                        // Built in the world, not typed: the blueprint reads it off the jack frame.
                        let blocks = game.placed(puzzle);
                        ui.horizontal(|ui| {
                            ui.label(format!("{}:", spec.label));
                            ui.label(RichText::new(format!("{blocks} {}", spec.unit)).strong().color(GOOD));
                        });
                        let plural = if blocks == 1 { "" } else { "s" };
                        ui.label(
                            RichText::new(format!(
                                "    {blocks} block{plural} in the jack frame at the edge of the ledge. Leave the console and carry blocks there (F) to change it."
                            ))
                            .small()
                            .color(DIM),
                        );
                        continue;
                    }
                    let field = ui
                        .horizontal(|ui| {
                            ui.label(format!("{}:", spec.label));
                            ui.add(egui::TextEdit::singleline(&mut ui_state.drafts[puzzle][i]).hint_text(spec.example).desired_width(170.0))
                        })
                        .inner;
                    if field.has_focus() {
                        ui_state.last_input = Some(InputTarget::Blueprint(puzzle, i));
                    }
                    if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        enter_commit = true;
                    }
                    let text = ui_state.drafts[puzzle][i].trim();
                    if !text.is_empty() {
                        match evaluate(text, &game.save.notebook) {
                            Ok(q) if q.dim == spec.dim => ui.colored_label(GOOD, format!("    = {}", q.display_in(spec.unit))),
                            Ok(q) => ui.colored_label(WARN, format!("    = {}, but this needs {}", q.display(), spec.dim.describe())),
                            Err(e) => ui.colored_label(BAD, format!("    {e}")),
                        };
                    }
                }
                let hint = match def.kind {
                    PuzzleKind::Rail => Some("Give every number its unit, as in 12 m. You can also type a notebook name, such as d1."),
                    _ if specs.iter().all(|s| s.source == InputSource::Placed) => None,
                    _ => Some("Give every number its unit. Notebook names and functions work too, as in (d1 + d2) / 2 or sin(30 deg)."),
                };
                if let Some(hint) = hint {
                    ui.label(RichText::new(hint).small().color(DIM));
                }

                let charges = game.charges_left(puzzle);
                let ready = !sim.busy() && charges > 0;
                let all_typed_filled = specs
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.source == InputSource::Typed)
                    .all(|(i, _)| !ui_state.drafts[puzzle][i].trim().is_empty());
                if enter_commit && ready && all_typed_filled {
                    sim.request = Some(Request::Commit(puzzle));
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if ui.add_enabled(ready, egui::Button::new(RichText::new("Commit and test").strong())).clicked() {
                        sim.request = Some(Request::Commit(puzzle));
                    }
                    match (charges, ui_state.recharging[puzzle]) {
                        (0, Some(left)) => ui.colored_label(WARN, format!("Recharging: {:.0} s", left.ceil())),
                        (0, None) => ui.colored_label(WARN, "Recharging"),
                        _ => ui.label(format!("Prototype charges: {charges} of {}", def.charges)),
                    };
                });
                if let Some(refusal) = &ui_state.refusal {
                    ui.colored_label(WARN, format!("Not launched, and no charge was used. {refusal}"));
                }
                if sim.testing() == Some(puzzle) {
                    ui.label(RichText::new("Test running…").italics());
                }

                if let Some(outcome) = &ui_state.results[puzzle] {
                    ui.separator();
                    result_section(ui, &instance, outcome);
                    if std::mem::take(&mut ui_state.scroll_to_result[puzzle]) {
                        ui.scroll_to_cursor(Some(egui::Align::BOTTOM));
                    }
                }

                let progress = game.save.progress(&def.id);
                if let Some(quality) = progress.quality() {
                    ui.separator();
                    let words = match quality {
                        Quality::Pristine => "pristine (first test)",
                        Quality::Serviceable => "serviceable",
                    };
                    ui.colored_label(GOOD, format!("Solved. Part quality: {words}."));
                    if game.save.chapter_complete(&game.chapter) {
                        ui.label(format!("The {} is in place.", game.chapter.part.to_lowercase()));
                    }
                }
                if progress.attempts.len() > 1 {
                    ui.collapsing(format!("All tests ({})", progress.attempts.len()), |ui| {
                        for (n, attempt) in progress.attempts.iter().enumerate() {
                            let values: Vec<String> = attempt.values.iter().map(|v| v.quantity.display_in(&v.unit)).collect();
                            let colour = if attempt.pass { GOOD } else { DIM };
                            ui.colored_label(colour, format!("{}. {}: {}", n + 1, values.join(", "), attempt.headline));
                        }
                    });
                }
            });
        })
    {
        record_rect(ui_state, resp.response.rect);
    }
}

/// The ledge launcher's rating plate: the band its damaged dial turns through, which can be noted
/// like a ruler reading.
fn rating_plate(ui: &mut egui::Ui, game: &mut Game) {
    let (low, high) = (Quantity::speed(game.layout.channel_speed_min), Quantity::speed(game.layout.channel_speed_max));
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("Rating plate: the damaged dial only turns from {} to {}.", low.display(), high.display())).strong());
        let noted = game.save.notebook.entries.iter().any(|e| e.source == source::PLATE);
        if ui.add_enabled(!noted, egui::Button::new("Note")).on_hover_text("Write both ends in the notebook, as vmin and vmax").clicked() {
            note_plate(game);
        }
    });
}

/// Write the ends of the ledge launcher's speed band into the notebook, as the plate's Note button does.
pub fn note_plate(game: &mut Game) {
    write_plate(game);
    game.persist();
}

/// Write the ends of the ledge launcher's speed band into the notebook, without saving.
pub fn write_plate(game: &mut Game) {
    let (low, high) = (Quantity::speed(game.layout.channel_speed_min), Quantity::speed(game.layout.channel_speed_max));
    game.save.notebook.set("vmin", "lowest speed of the ledge launcher's dial", low, source::PLATE);
    game.save.notebook.set("vmax", "highest speed of the ledge launcher's dial", high, source::PLATE);
}

/// The station's free conceptual demonstration: a guess first, then the world shows the principle.
/// Measurements live in [`measure_section`] below, separate from this.
fn experiment_section(ui: &mut egui::Ui, game: &Game, ui_state: &mut Ui, sim: &mut Sim, puzzle: usize, demo: &DemoDef) {
    let id = &game.def(puzzle).id;
    let watched = game.save.demo_watched(id);
    let running = sim.demo() == Some(puzzle);
    ui.label(&demo.question);
    let first_guess = game.save.first_guess(id);
    for (i, choice) in demo.choices.iter().enumerate() {
        if watched {
            // The guess before the first run stays on record, beside what happened. One label per
            // choice, so it wraps inside the panel.
            let (mark, note, colour) = match (i == demo.answer, first_guess == Some(i)) {
                (true, true) => ("✔", "  (your guess)", GOOD),
                (true, false) => ("✔", "", Color32::WHITE),
                (false, true) => ("✖", "  (your guess)", WARN),
                (false, false) => ("·", "", DIM),
            };
            ui.label(RichText::new(format!("{mark}  {choice}{note}")).color(colour));
        } else {
            if ui.add_enabled(!running, egui::RadioButton::new(ui_state.guess[puzzle] == Some(i), choice.as_str())).clicked() {
                ui_state.guess[puzzle] = Some(i);
            }
        }
    }
    ui.horizontal(|ui| {
        let ready = !sim.busy() && (watched || ui_state.guess[puzzle].is_some());
        let label = if watched { "Run it again" } else { "Run the experiment" };
        if ui.add_enabled(ready, egui::Button::new(label)).clicked() {
            sim.request = Some(Request::Demo(puzzle));
        }
        if running {
            ui.label(RichText::new("Watch…").italics());
        } else if !watched && ui_state.guess[puzzle].is_none() {
            ui.label(RichText::new("Make a guess first. It costs nothing.").small().color(DIM));
        } else {
            ui.label(RichText::new("It's free, so run it as often as you like.").small().color(DIM));
        }
    });
    if watched && !running {
        ui.label(RichText::new(&demo.reveal).color(Color32::from_rgb(200, 225, 240)));
    }
}

/// The station's built-in measuring instrument (stopwatch at the haul track, light gate at the drop
/// tower): free, repeatable, and separate from the conceptual "Try it first" demonstration.
fn measure_section(ui: &mut egui::Ui, game: &Game, sim: &mut Sim, puzzle: usize) {
    let kind = game.def(puzzle).kind;
    let measuring = sim.measuring() == Some(puzzle);
    match kind {
        PuzzleKind::Chase => {
            ui.label(RichText::new("Measure · Stopwatch").strong());
            ui.horizontal(|ui| {
                if ui.add_enabled(!sim.busy(), egui::Button::new(RichText::new("Time the crawler").strong())).clicked() {
                    sim.request = Some(Request::Measure(puzzle));
                }
                if measuring {
                    ui.label(RichText::new("Timing…").italics());
                } else {
                    ui.label(RichText::new("Free. Times the crawler between the two posts.").small().color(DIM));
                }
            });
            match game.save.notebook.entries.iter().rev().find(|e| e.label == CRAWLER_TIME) {
                Some(time) => ui.label(RichText::new(format!("Post A to post B: {} = {}", time.name, time.quantity.display())).color(GOOD)),
                None => ui.label(RichText::new("No reading yet.").small().color(DIM)),
            };
        }
        PuzzleKind::Gravity => {
            ui.label(RichText::new("Measure · Light gate").strong());
            let gate = game.placed(puzzle);
            let unready = game.instance(puzzle).measure_unready(gate);
            if gate > 0 {
                let built = game.instance(puzzle).built(gate).unwrap_or_default();
                ui.label(RichText::new(format!("{}.", capitalise(&built))).color(GOOD));
            } else {
                ui.horizontal(|ui| {
                    ui.label("Light gate:");
                    ui.label(RichText::new("not fitted").strong().color(WARN));
                });
                if let Some(how) = &unready {
                    ui.label(RichText::new(format!("    {how}")).small().color(DIM));
                }
            }
            ui.horizontal(|ui| {
                let ready = !sim.busy() && unready.is_none();
                if ui.add_enabled(ready, egui::Button::new(RichText::new("Drop and time").strong())).clicked() {
                    sim.request = Some(Request::Measure(puzzle));
                }
                if measuring {
                    ui.label(RichText::new("Dropping…").italics());
                } else if unready.is_some() {
                    ui.label(RichText::new("Clamp the light gate first.").small().color(WARN));
                } else {
                    ui.label(RichText::new("Free. Move the gate to another notch for another pair.").small().color(DIM));
                }
            });
            let pairs: Vec<String> = game
                .save
                .notebook
                .entries
                .iter()
                .filter(|e| e.source == source::LIGHT_GATE && e.quantity.dim == physics_core::units::Dim::TIME)
                .map(|time| {
                    let number = time.name.trim_start_matches('t');
                    let drop = game.save.notebook.get(&format!("h{number}")).map_or(String::new(), |h| format!("{} = {}, ", h.name, h.quantity.display()));
                    format!("{drop}{} = {}", time.name, time.quantity.display())
                })
                .collect();
            if pairs.is_empty() {
                ui.label(RichText::new("No readings yet.").small().color(DIM));
            } else {
                for line in pairs {
                    ui.label(RichText::new(line).color(GOOD));
                }
            }
        }
        _ => {}
    }
}

/// A sentence's first letter in capitals.
fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or(String::new(), |first| first.to_uppercase().chain(chars).collect())
}

/// The handbook pages a station's job uses: a button for each found page, the hint for a locked one.
fn pages_row(ui: &mut egui::Ui, game: &Game, ui_state: &mut Ui, def: &PuzzleDef) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new("Handbook:").color(DIM));
        for concept in def.concepts.iter().filter_map(|id| game.chapter.concept(id)) {
            if game.page_found(&concept.id) {
                let new = ui_state.unread_pages.contains(&concept.id);
                let text = if new { RichText::new(format!("{} · new", concept.title)).color(GOOD) } else { RichText::new(&concept.title) };
                if ui.small_button(text).on_hover_text("Open this page in the handbook").clicked() {
                    ui_state.open_page(&concept.id);
                }
            } else {
                ui.label(RichText::new(format!("{} (not found yet)", concept.title)).small().italics().color(DIM)).on_hover_text(&concept.hint);
            }
        }
    });
}

fn result_section(ui: &mut egui::Ui, instance: &Instance, outcome: &Outcome) {
    ui.label(RichText::new(&outcome.headline).strong().size(17.0).color(if outcome.pass { GOOD } else { BAD }));
    ui.label(&outcome.detail);
    if !outcome.comparisons.is_empty() {
        let is_drop = matches!(outcome.playback, Playback::Drop { .. });
        egui::Grid::new("comparisons").num_columns(3).striped(true).show(ui, |ui| {
            ui.label("");
            ui.label(RichText::new(if is_drop { "your prediction" } else { "target" }).color(DIM));
            ui.label(RichText::new("observed").color(DIM));
            ui.end_row();
            for row in &outcome.comparisons {
                ui.label(&row.label);
                ui.label(row.expected.display());
                ui.label(row.observed.display());
                ui.end_row();
            }
        });
    }
    if let Playback::Flight { projectile, flight } = &outcome.playback
        && let Some(profile) = instance.profile()
    {
        // Side view: terrain, target, and the path that was flown.
        let layout = &instance.layout;
        let (target, half) = match instance.kind {
            PuzzleKind::Channel => (layout.platform_centre, layout.platform_half),
            _ => (layout.pad_centre, layout.pad_half),
        };
        let x_min = -3.0;
        let x_max = (target + half + 6.0).max(flight.x + 3.0).min(profile.x_end());
        let (_, _, apex) = projectile.apex();
        let y_min = if instance.kind == PuzzleKind::Channel { -Ch1Layout::WATER_DROP - 1.0 } else { -2.0 };
        let y_max = apex.max(layout.ridge_h as f64 * (instance.kind == PuzzleKind::Ridge) as u8 as f64) + 2.0;

        let width = ui.available_width().min(400.0);
        let scale = (width as f64 / (x_max - x_min)).min(170.0 / (y_max - y_min));
        let size = egui::vec2(width, ((y_max - y_min) * scale) as f32);
        let (response, painter) = ui.allocate_painter(size, egui::Sense::hover());
        let rect = response.rect;
        let at = |x: f64, y: f64| egui::pos2(rect.left() + ((x - x_min) * scale) as f32, rect.bottom() - ((y - y_min) * scale) as f32);
        painter.rect_filled(rect, 3.0, Color32::from_rgb(24, 30, 42));

        let ground = Color32::from_rgb(86, 96, 78);
        let launch_height = projectile.y0;
        painter.rect_filled(egui::Rect::from_two_pos(at(x_min, launch_height), at(0.0, y_min)), 0.0, ground);
        for (i, height) in profile.heights.iter().enumerate() {
            let left = profile.x_start + i as f64;
            if left >= x_max {
                break;
            }
            painter.rect_filled(egui::Rect::from_two_pos(at(left, *height), at((left + 1.0).min(x_max), y_min)), 0.0, ground);
        }
        painter.rect_filled(egui::Rect::from_two_pos(at(target - half, 0.0), at(target + half, -0.8)), 0.0, Color32::from_rgb(240, 140, 20));

        let steps = 60;
        let path: Vec<egui::Pos2> = (0..=steps)
            .map(|i| {
                let (x, y) = projectile.pos(flight.t * i as f64 / steps as f64);
                at(x, y)
            })
            .collect();
        painter.add(egui::Shape::line(path, egui::Stroke::new(2.0, Color32::from_rgb(255, 217, 51))));
        let end_colour = if flight.end == FlightEnd::Landed && outcome.pass { GOOD } else { BAD };
        painter.circle_filled(at(flight.x, flight.y), 4.0, end_colour);
        ui.label(RichText::new("Side view, to scale. Orange: target.").small().color(DIM));
    }
    if let Playback::Chase { cart_speed, crawler_speed, head_start, t } = outcome.playback {
        chase_graph(ui, &instance.layout, cart_speed, crawler_speed, head_start, t, outcome.pass);
    }
}

/// Position along the haul track against time, for the cart and the crawler: two straight lines,
/// since both run at steady speeds, that meet where the cart reached the crawler. The marker is a band.
fn chase_graph(ui: &mut egui::Ui, layout: &Ch1Layout, cart_speed: f64, crawler_speed: f64, head_start: f64, end: f64, pass: bool) {
    let x_max = Ch1Layout::HAUL_LENGTH as f64 + 1.0;
    let t_max = end * 1.08 + 0.5;
    let width = ui.available_width().min(400.0);
    let height = 170.0;
    let (response, painter) = ui.allocate_painter(egui::vec2(width, height), egui::Sense::hover());
    let rect = response.rect.shrink2(egui::vec2(6.0, 6.0));
    let at = |t: f64, x: f64| egui::pos2(rect.left() + (t / t_max) as f32 * rect.width(), rect.bottom() - (x / x_max) as f32 * rect.height());
    painter.rect_filled(response.rect, 3.0, Color32::from_rgb(24, 30, 42));
    let axis = egui::Stroke::new(1.0, Color32::from_rgb(90, 100, 120));
    painter.line_segment([at(0.0, 0.0), at(t_max, 0.0)], axis);
    painter.line_segment([at(0.0, 0.0), at(0.0, x_max)], axis);
    let (m, half) = (layout.transfer_at as f64, Ch1Layout::TRANSFER_HALF);
    painter.rect_filled(egui::Rect::from_two_pos(at(0.0, m - half), at(t_max, m + half)), 0.0, Color32::from_rgba_unmultiplied(240, 140, 20, 90));
    let crawler = Color32::from_rgb(235, 200, 56);
    let cart = Color32::from_rgb(255, 120, 100);
    painter.line_segment([at(0.0, head_start), at(end, head_start + crawler_speed * end)], egui::Stroke::new(2.0, crawler));
    painter.line_segment([at(0.0, 0.0), at(end, cart_speed * end)], egui::Stroke::new(2.0, cart));
    painter.circle_filled(at(end, cart_speed * end), 4.0, if pass { GOOD } else { BAD });
    ui.label(RichText::new("Distance along the track against time. Red: the cart. Yellow: the crawler. Orange: the marker.").small().color(DIM));
}

/// Compact floating label near the hovered pin or block face while the ruler is aiming.
fn ruler_hover_label(ctx: &egui::Context, ruler: &Ruler) {
    let (Some(hover), Some(screen_px)) = (&ruler.hover, ruler.hover_screen) else { return };
    let zoom = ctx.zoom_factor();
    let screen = ctx.content_rect();
    let pos = egui::pos2(screen_px.x / zoom, screen_px.y / zoom);
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("ruler-hover")));
    let title_colour = if hover.snapped { Color32::from_rgb(255, 232, 130) } else { Color32::from_rgb(230, 235, 245) };
    let border_colour = if hover.snapped { WARN } else { Color32::from_rgb(120, 135, 155) };
    let title = painter.layout_no_wrap(hover.label.clone(), egui::FontId::proportional(13.0), title_colour);
    let live = if ruler.first.is_some() && ruler.second.is_none() {
        ruler.preview().map(|m| {
            let line = format!("Horizontal: {}   Vertical: {}", Quantity::metres(m.horizontal as f64).display(), Quantity::metres(m.vertical as f64).display());
            painter.layout_no_wrap(line, egui::FontId::proportional(11.5), Color32::from_rgb(170, 235, 250))
        })
    } else {
        None
    };
    let width = title.size().x.max(live.as_ref().map_or(0.0, |g| g.size().x));
    let height = title.size().y + live.as_ref().map_or(0.0, |g| g.size().y + 2.0);
    let size = egui::vec2(width + 14.0, height + 8.0);
    let mut rect = egui::Rect::from_min_size(pos + egui::vec2(14.0, -size.y - 10.0), size);
    rect = rect.translate(egui::vec2(
        (screen.left() + 8.0 - rect.left()).max(0.0) - (rect.right() - screen.right() + 8.0).max(0.0),
        (screen.top() + 8.0 - rect.top()).max(0.0) - (rect.bottom() - screen.bottom() + 8.0).max(0.0),
    ));
    painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(12, 20, 32, 220));
    painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0, border_colour), egui::StrokeKind::Inside);
    painter.galley(rect.min + egui::vec2(7.0, 4.0), title, Color32::WHITE);
    if let Some(live) = live {
        painter.galley(rect.min + egui::vec2(7.0, 4.0 + height - live.size().y), live, Color32::WHITE);
    }
}

fn ruler_panel(ctx: &egui::Context, game: &mut Game, ui_state: &mut Ui, ruler: &mut Ruler) {
    let mut open = true;
    if let Some(resp) =
        egui::Window::new("Ruler").open(&mut open).collapsible(false).resizable(false).anchor(Align2::LEFT_BOTTOM, [10.0, -10.0]).show(ctx, |ui| {
            let hover = ruler.hover.as_ref().map(|h| h.label.as_str());
            match (&ruler.first, &ruler.second) {
                (None, _) => {
                    ui.label("Click the first point.");
                    ui.label(RichText::new(hover.unwrap_or("Yellow pins mark surveyed points.")).color(DIM));
                }
                (Some(first), None) => {
                    ui.label(format!("From: {}", first.label));
                    ui.label(RichText::new(format!("To: {}", hover.unwrap_or("click the second point"))).color(DIM));
                    if let Some(m) = ruler.preview() {
                        ui.separator();
                        egui::Grid::new("ruler-preview").num_columns(2).show(ui, |ui| {
                            for (word, value) in [("Horizontal", m.horizontal), ("Vertical", m.vertical), ("Direct", m.direct)] {
                                ui.label(RichText::new(word).color(DIM));
                                ui.label(RichText::new(Quantity::metres(value as f64).display()).color(Color32::from_rgb(180, 235, 250)));
                                ui.end_row();
                            }
                        });
                        ui.label(RichText::new("Click to lock this reading.").small().color(DIM));
                    }
                }
                (Some(first), Some(second)) => {
                    ui.label(format!("From: {}", first.label));
                    ui.label(format!("To: {}", second.label));
                }
            }
            let (Some(m), Some(first), Some(second)) = (ruler.measurement(), ruler.first.clone(), ruler.second.clone()) else { return };
            ui.separator();
            let between = format!("{} to {}", first.label, second.label);
            let mut saved = None;
            egui::Grid::new("ruler").num_columns(3).show(ui, |ui| {
                for (idx, (word, prefix, value)) in
                    [("Horizontal", "d", m.horizontal), ("Vertical", "h", m.vertical), ("Direct", "s", m.direct)].into_iter().enumerate()
                {
                    ui.label(word);
                    ui.label(RichText::new(Quantity::metres(value as f64).display()).strong());
                    let existing = ruler.noted[idx].as_deref().filter(|name| game.save.notebook.get(name).is_some());
                    if let Some(name) = existing {
                        ui.colored_label(GOOD, format!("Noted as {name}"));
                    } else if ui.button(format!("Note as {prefix}…")).clicked() {
                        saved = Some((idx, prefix, word, value));
                    }
                    ui.end_row();
                }
            });
            if let Some((idx, prefix, word, value)) = saved {
                let label = format!("{} distance, {between}", word.to_lowercase());
                let name = game.save.notebook.add_numbered(prefix, &label, Quantity::metres(value as f64), source::RULER);
                ruler.noted[idx] = Some(name);
                game.persist();
            }
            ui.label(RichText::new("R measures again · Esc puts the ruler away").small().color(DIM));
        })
    {
        record_rect(ui_state, resp.response.rect);
    }
    if !open {
        ruler.put_away();
    }
}

/// How many calculator expressions are kept for Up/Down recall.
const MAX_CALC_HISTORY: usize = 30;

fn push_history(history: &mut Vec<String>, entry: &str) {
    let trimmed = entry.trim();
    if trimmed.is_empty() || history.last().map(String::as_str) == Some(trimmed) {
        return;
    }
    history.push(trimmed.to_string());
    if history.len() > MAX_CALC_HISTORY {
        let drop = history.len() - MAX_CALC_HISTORY;
        history.drain(..drop);
    }
}

/// Insert a notebook variable name into the most recently focused blueprint or calculator field.
fn insert_into_target(ui_state: &mut Ui, name: &str) {
    let target = match ui_state.last_input {
        Some(InputTarget::Blueprint(puzzle, idx)) if ui_state.station == Some(puzzle) && idx < ui_state.drafts[puzzle].len() => {
            &mut ui_state.drafts[puzzle][idx]
        }
        _ => {
            ui_state.calc_history_idx = None;
            &mut ui_state.calculator
        }
    };
    if target.chars().last().is_some_and(|c| c.is_alphanumeric() || c == '_') {
        target.push(' ');
    }
    target.push_str(name);
}

fn notebook_body(ui: &mut egui::Ui, game: &mut Game, ui_state: &mut Ui, room: f32) {
    if game.save.notebook.entries.is_empty() {
        ui.label(RichText::new("Nothing measured yet. Readings from the ruler and the stopwatch will show up here.").color(DIM));
    }
    let mut remove = None;
    let mut insert = None;
    let mut start_rename = None;
    let mut commit_rename: Option<(String, String)> = None;
    let mut cancel_rename = false;
    let mut rename_error: Option<String> = None;
    // The calculator under the entries needs about this much.
    let entries_room = (room - 105.0).max(30.0);
    egui::ScrollArea::vertical().id_salt("notebook-entries").max_height(entries_room).show(ui, |ui| {
        egui::Grid::new("notebook").num_columns(4).striped(true).show(ui, |ui| {
            for entry in &game.save.notebook.entries {
                let editing = ui_state.renaming_entry.as_ref().is_some_and(|(orig, _)| orig == &entry.name);
                if editing {
                    let (orig, draft) = ui_state.renaming_entry.as_mut().expect("checked above");
                    let edit = ui.add(egui::TextEdit::singleline(draft).desired_width(64.0));
                    let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let esc = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape));
                    ui.label(entry.quantity.display());
                    ui.label(RichText::new(&entry.label).small().color(DIM)).on_hover_text(format!("Source: {}", entry.source));
                    ui.horizontal(|ui| {
                        if ui.small_button("Save").clicked() || enter {
                            commit_rename = Some((orig.clone(), draft.trim().to_string()));
                        }
                        if ui.small_button("Cancel").clicked() || esc {
                            cancel_rename = true;
                        }
                    });
                    if !draft.trim().is_empty() && draft.trim() != orig.as_str() {
                        let candidate = draft.trim();
                        if !valid_name(candidate) {
                            rename_error =
                                Some(format!("'{candidate}' cannot be a notebook name: use letters and digits, starting with a letter, and not a unit."));
                        } else if game.save.notebook.get(candidate).is_some() {
                            rename_error = Some(format!("'{candidate}' is already in the notebook."));
                        }
                    }
                } else {
                    if ui
                        .small_button(RichText::new(&entry.name).strong().monospace())
                        .on_hover_text("Click to insert into the calculator or blueprint")
                        .clicked()
                    {
                        insert = Some(entry.name.clone());
                    }
                    ui.label(entry.quantity.display());
                    ui.label(RichText::new(&entry.label).small().color(DIM)).on_hover_text(format!("Source: {}", entry.source));
                    ui.horizontal(|ui| {
                        if ui.small_button("Rename").on_hover_text("Rename this entry").clicked() {
                            start_rename = Some(entry.name.clone());
                        }
                        if ui.small_button("×").on_hover_text("Remove this entry").clicked() {
                            remove = Some(entry.name.clone());
                        }
                    });
                }
                ui.end_row();
            }
        });
    });
    if let Some(err) = rename_error {
        ui.colored_label(WARN, err);
    }
    if cancel_rename {
        ui_state.renaming_entry = None;
    } else if let Some((orig, new_name)) = commit_rename {
        if new_name == orig {
            ui_state.renaming_entry = None;
        } else if valid_name(&new_name) && game.save.notebook.rename(&orig, &new_name) {
            game.persist();
            ui_state.renaming_entry = None;
        }
    } else if let Some(name) = start_rename {
        ui_state.renaming_entry = Some((name.clone(), name));
    }
    if let Some(name) = insert {
        insert_into_target(ui_state, &name);
    }
    if let Some(name) = remove {
        if ui_state.renaming_entry.as_ref().is_some_and(|(orig, _)| orig == &name) {
            ui_state.renaming_entry = None;
        }
        game.save.notebook.remove(&name);
        game.persist();
    }

    ui.separator();
    ui.label(RichText::new("Calculator").strong());
    let calc_field = ui.add(egui::TextEdit::singleline(&mut ui_state.calculator).hint_text("(t1 + t2) / 2   or   tavg = (t1 + t2) / 2").desired_width(360.0));
    if calc_field.changed() {
        ui_state.calc_history_idx = None;
    }
    if calc_field.has_focus() {
        ui_state.last_input = Some(InputTarget::Calculator);
        if !ui_state.calc_history.is_empty() {
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                let next = match ui_state.calc_history_idx {
                    None => {
                        ui_state.calc_draft_saved = ui_state.calculator.clone();
                        ui_state.calc_history.len() - 1
                    }
                    Some(idx) => idx.saturating_sub(1),
                };
                ui_state.calc_history_idx = Some(next);
                ui_state.calculator = ui_state.calc_history[next].clone();
            } else if ui.input(|i| i.key_pressed(egui::Key::ArrowDown))
                && let Some(idx) = ui_state.calc_history_idx
            {
                if idx + 1 < ui_state.calc_history.len() {
                    let next = idx + 1;
                    ui_state.calc_history_idx = Some(next);
                    ui_state.calculator = ui_state.calc_history[next].clone();
                } else {
                    ui_state.calc_history_idx = None;
                    ui_state.calculator = std::mem::take(&mut ui_state.calc_draft_saved);
                }
            }
        }
    }
    let submitted = calc_field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
    let text = ui_state.calculator.trim().to_string();
    // "name = expression" keeps the result in the notebook.
    let (name, expression) = match text.split_once('=') {
        Some((name, expression)) => (Some(name.trim().to_string()), expression.trim().to_string()),
        None => (None, text.clone()),
    };
    if !expression.is_empty() {
        match evaluate(&expression, &game.save.notebook) {
            Ok(q) => {
                ui.horizontal(|ui| {
                    ui.colored_label(GOOD, format!("= {}", q.display()));
                    if q.dim.is_none() {
                        ui.label(RichText::new(format!("(as an angle: {} deg)", sig(q.value.to_degrees(), 4))).small().color(DIM));
                    }
                });
                match name {
                    Some(name) if valid_name(&name) => {
                        if ui.button(format!("Note as {name}")).clicked() || submitted {
                            game.save.notebook.set(&name, &expression, q, source::CALCULATED);
                            push_history(&mut ui_state.calc_history, &text);
                            game.persist();
                            ui_state.calculator.clear();
                            ui_state.calc_history_idx = None;
                            if submitted {
                                calc_field.request_focus();
                            }
                        }
                    }
                    Some(name) => {
                        ui.colored_label(WARN, format!("'{name}' cannot be a notebook name: use letters and digits, starting with a letter, and not a unit."));
                    }
                    None => {
                        if submitted {
                            push_history(&mut ui_state.calc_history, &text);
                            ui_state.calc_history_idx = None;
                            calc_field.request_focus();
                        }
                        let tip = if ui_state.calc_history.is_empty() {
                            "Write  name = expression  and press Enter to keep a result."
                        } else {
                            "Write  name = expression  and press Enter to keep a result. Up and Down recall past entries."
                        };
                        ui.label(RichText::new(tip).small().color(DIM));
                    }
                }
            }
            Err(e) => {
                ui.colored_label(BAD, e.to_string());
            }
        }
    } else if !ui_state.calc_history.is_empty() {
        ui.label(RichText::new("Press Up and Down to recall past entries.").small().color(DIM));
    }
}

/// A notebook name must read as a variable and must not hide a unit or a function.
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let starts_well = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    starts_well && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') && !is_function(name) && (name == "g" || lookup_unit(name).is_none())
}

/// The conversation with Lumen: drawn by the companion inside the panel that grows out of its eye.
pub fn tutor_body(ui: &mut egui::Ui, game: &Game, ui_state: &mut Ui, lumen: &mut Lumen, landmarks: &Landmarks, history: &[String], room: f32) {
    let puzzle = ui_state.focus(game);
    ui.set_width(430.0);
    if !lumen.tutor.is_online() {
        ui.label(RichText::new("Recent lines").strong());
        if history.is_empty() {
            ui.label(RichText::new("Lumen hasn't said anything yet this session.").small().color(DIM));
        } else {
            let log_room = (room - 70.0).max(60.0);
            egui::ScrollArea::vertical().id_salt("lumen-history-offline").max_height(log_room).stick_to_bottom(true).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for line in history {
                    ui.label(RichText::new(line).color(LUMEN));
                    ui.add_space(4.0);
                }
            });
        }
        ui.separator();
        ui.colored_label(WARN, "Lumen's chat is offline.");
        ui.label(RichText::new(lumen.tutor.status()).small().color(DIM));
        ui.label(
            RichText::new("To ask questions in chat, add a [tutor] section to settings.toml (see docs/tutor.md). The handbook (H) works without one.")
                .small()
                .color(DIM),
        );
        return;
    }
    ui.label(RichText::new(format!("Looking at: {} · {}", game.def(puzzle).title, lumen.tutor.status())).small().color(DIM));
    ui.label(RichText::new("Lumen can see this puzzle and explain it, but it can't do anything in the world.").small().color(DIM));
    if !history.is_empty() {
        egui::CollapsingHeader::new(RichText::new(format!("Recent lines ({})", history.len())).small().color(DIM)).id_salt("lumen-history-online").show(
            ui,
            |ui| {
                egui::ScrollArea::vertical().id_salt("lumen-history-scroll").max_height(110.0).stick_to_bottom(true).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for line in history {
                        ui.label(RichText::new(line).small().color(LUMEN));
                        ui.add_space(2.0);
                    }
                });
            },
        );
    }
    let conversation = &lumen.conversations[puzzle];
    let (mut send, mut stop) = (false, false);
    egui::ScrollArea::vertical().id_salt("lumen-conversation").max_height(room).stick_to_bottom(true).show(ui, |ui| {
        ui.set_width(ui.available_width());
        for turn in &conversation.turns {
            match turn {
                Turn::Player(text) => ui.label(RichText::new(format!("You: {text}")).color(Color32::from_rgb(150, 200, 255))),
                Turn::Tutor(text) => ui.label(text),
                Turn::Problem(text) => ui.colored_label(BAD, text),
            };
            ui.add_space(4.0);
        }
        if let Some(arriving) = conversation.arriving() {
            ui.label(if arriving.is_empty() { "…".to_string() } else { format!("{arriving} …") });
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        let field = ui.add(egui::TextEdit::singleline(&mut ui_state.chat).hint_text("Ask about the physics").desired_width(330.0));
        let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if conversation.is_waiting() {
            stop = ui.button("Stop").on_hover_text("Stop waiting for this reply").clicked();
        } else {
            send = entered || ui.button("Ask").clicked();
        }
    });
    if stop {
        lumen.conversations[puzzle].cancel();
    }
    if send && !ui_state.chat.trim().is_empty() {
        let text = std::mem::take(&mut ui_state.chat);
        lumen.ask(game, ui_state, landmarks, puzzle, &text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clicking_a_notebook_name_inserts_into_the_focused_field() {
        let mut ui = Ui { station: Some(2), ..Ui::default() };
        ui.drafts[2] = vec!["2 * ".into()];
        ui.last_input = Some(InputTarget::Blueprint(2, 0));
        insert_into_target(&mut ui, "h1");
        assert_eq!(ui.drafts[2][0], "2 * h1");
        // Adjacent identifier gets a separating space.
        insert_into_target(&mut ui, "g");
        assert_eq!(ui.drafts[2][0], "2 * h1 g");
        // Leaving the station falls back to the calculator.
        ui.station = None;
        insert_into_target(&mut ui, "d1");
        assert_eq!(ui.calculator, "d1");
    }

    #[test]
    fn calculator_history_deduplicates_consecutive_entries_and_caps() {
        let mut history = Vec::new();
        push_history(&mut history, "  ");
        assert!(history.is_empty());
        push_history(&mut history, "d1 / t1");
        push_history(&mut history, "d1 / t1");
        assert_eq!(history, ["d1 / t1"]);
        for i in 0..40 {
            push_history(&mut history, &format!("x = {i} m"));
        }
        assert_eq!(history.len(), MAX_CALC_HISTORY);
        assert_eq!(history.last().unwrap(), "x = 39 m");
    }
}
