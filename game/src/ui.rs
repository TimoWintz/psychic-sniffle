//! Interface panels: the dock at the top left (heads-up display, notebook and calculator), the
//! handbook beside it,
//! the station, the ruler's reading, and the body of the conversation with Lumen (drawn by the
//! companion, attached to Lumen's eye).
//!
//! The notebook, the handbook and Lumen are always on screen. Reduced, they show only their header
//! (Lumen: its eye); their key, or a click on the header, expands and reduces them. The handbook has
//! its own place beside the dock's column, where it opens downward to read at full height.

use crate::carry::Carry;
use crate::companion::Landmarks;
use crate::ruler::Ruler;
use crate::sim::{Request, Sim};
use crate::state::{Game, Mode, Ui};
use crate::tutor_bridge::Lumen;
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use egui::{Align2, Color32, RichText};
use physics_core::ballistics::FlightEnd;
use physics_core::expr::evaluate;
use physics_core::content::{DemoDef, PuzzleDef};
use physics_core::materials::Material;
use physics_core::progress::Quality;
use physics_core::puzzle::{InputSource, Instance, Outcome, Playback, PuzzleKind};
use physics_core::units::{Quantity, lookup_unit, sig};
use tutor::Turn;

const GOOD: Color32 = Color32::from_rgb(110, 220, 130);
const WARN: Color32 = Color32::from_rgb(240, 190, 80);
const BAD: Color32 = Color32::from_rgb(240, 110, 100);
const DIM: Color32 = Color32::from_rgb(160, 165, 175);

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

fn hotkeys(keys: Res<ButtonInput<KeyCode>>, wants: Res<EguiWantsInput>, mut ui: ResMut<Ui>, mut ruler: ResMut<Ruler>) {
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
    if keys.just_pressed(KeyCode::Escape) {
        // One thing at a time: the ruler first, then the station.
        if ruler.in_use() {
            ruler.put_away();
        } else {
            ui.station = None;
        }
    }
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
    mut handbook_was_open: Local<bool>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    // Reborrow once so the panels can borrow fields separately.
    let (game, ui_state, sim, ruler) = (&mut *game, &mut *ui_state, &mut *sim, &mut *ruler);

    dock(&ctx, game, ui_state, ruler);
    hints(&ctx, game, ui_state, ruler, &carry);
    if let Some(puzzle) = ui_state.station {
        station_panel(&ctx, game, ui_state, sim, puzzle);
    }
    if ruler.in_use() {
        ruler_panel(&ctx, game, ruler);
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
    let reserve = DOCK_BOTTOM_RESERVE
        + if ruler.in_use() { DOCK_RULER_RESERVE } else { 0.0 }
        + if ui_state.tutor_open { DOCK_LUMEN_RESERVE } else { 0.0 };
    let floor = screen.bottom() - reserve;
    let area = egui::Area::new(egui::Id::new("dock")).anchor(Align2::LEFT_TOP, [10.0, 10.0]).movable(false).show(ctx, |ui| {
        ui.spacing_mut().item_spacing.y = 6.0;
        egui::Frame::window(ui.style()).show(ui, |ui| {
            ui.set_max_width(DOCK_WIDTH);
            ui.label(RichText::new(format!("{}  {}", game.chapter.id, game.chapter.title)).strong());
            let marks: String = (0..game.chapter.puzzles.len())
                .map(|i| if game.solved(i) { "■ " } else if game.unlocked(i) { "□ " } else { "· " })
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
    handbook_panel(ctx, game, ui_state);
}

/// The handbook, at its place beside the dock's column. Reduced, it is only its header; expanded, it
/// runs down from there to the hints.
fn handbook_panel(ctx: &egui::Context, game: &Game, ui_state: &mut Ui) {
    let screen = ctx.content_rect();
    let left = HANDBOOK_LEFT;
    egui::Area::new(egui::Id::new("handbook")).anchor(Align2::LEFT_TOP, [left, 10.0]).movable(false).show(ctx, |ui| {
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
                ui.allocate_ui(egui::vec2(ui.available_width(), room), |ui| handbook_body(ui, game, ui_state, room));
            }
        });
    });
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

fn hints(ctx: &egui::Context, game: &Game, ui_state: &Ui, ruler: &Ruler, carry: &Carry) {
    let prompt = if ruler.active {
        Some("Measuring: left-click two points. R or Esc cancels.".to_string())
    } else if let Some(line) = crate::carry::hint(carry).filter(|_| ui_state.station.is_none()) {
        Some(line)
    } else {
        match (ui_state.near_station, ui_state.station) {
            (_, Some(_)) => Some("At the station. E or Esc to leave.".to_string()),
            (Some(near), None) if game.unlocked(near) => Some(format!("E — use station: {}", game.def(near).title)),
            (Some(near), None) => {
                let needs: Vec<&str> = game.def(near).requires.iter().filter_map(|id| game.chapter.puzzle(id)).map(|p| p.title.as_str()).collect();
                Some(format!("Station locked. Solve first: {}", needs.join(", ")))
            }
            _ => None,
        }
    };
    egui::Window::new("hints")
        .title_bar(false)
        .resizable(false)
        .anchor(Align2::CENTER_BOTTOM, [0.0, -10.0])
        .show(ctx, |ui| {
            if let Some(prompt) = prompt {
                ui.vertical_centered(|ui| ui.label(RichText::new(prompt).strong().color(WARN)));
            }
            if !ui_state.unread_pages.is_empty() && !ui_state.handbook_open {
                let line = match ui_state.unread_pages.as_slice() {
                    [one] => format!("New handbook page: {}. Press H to read it.", game.chapter.concept(one).map_or(one.as_str(), |c| c.title.as_str())),
                    many => format!("{} new handbook pages. Press H to read them.", many.len()),
                };
                ui.vertical_centered(|ui| ui.label(RichText::new(line).strong().color(GOOD)));
            }
            ui.label(
                RichText::new("WASD move · Shift run · Space jump · right-drag or arrows look · scroll zoom · E station · F carry · R measure · N notebook · H handbook · T or click Lumen")
                    .small()
                    .color(DIM),
            );
        });
}

fn station_panel(ctx: &egui::Context, game: &mut Game, ui_state: &mut Ui, sim: &mut Sim, puzzle: usize) {
    let def = game.def(puzzle).clone();
    let instance = game.instance(puzzle);
    let specs = instance.inputs();
    ui_state.drafts[puzzle].resize(specs.len(), String::new());

    egui::Window::new(format!("Station — {}", def.title))
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
                ui.label(RichText::new("You stay at the console while it is open.").small().color(DIM));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Leave (E)").clicked() {
                        ui_state.station = None;
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().max_height(ctx.content_rect().height() - 110.0).show(ui, |ui| {
                ui.label(&def.brief);
                if def.kind == PuzzleKind::Ridge {
                    ui.label(RichText::new(format!("Launcher rating: {} maximum.", Quantity::speed(game.layout.v_max).display())).strong());
                }
                if def.kind == PuzzleKind::Canyon {
                    rating_plate(ui, game);
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Investigate").strong());
                ui.label(RichText::new(&def.investigate).color(DIM));

                if let Some(demo) = &def.demo {
                    ui.add_space(6.0);
                    experiment_section(ui, game, ui_state, sim, puzzle, demo);
                }
                ui.add_space(4.0);
                pages_row(ui, game, ui_state, &def);

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
                for (i, spec) in specs.iter().enumerate() {
                    if spec.source == InputSource::Placed {
                        // Built in the world, not typed: the blueprint reads it off the jack frame.
                        let crates = game.placed(puzzle);
                        ui.horizontal(|ui| {
                            ui.label(format!("{}:", spec.label));
                            ui.label(RichText::new(format!("{crates} {}", spec.unit)).strong().color(GOOD));
                        });
                        let plural = if crates == 1 { "" } else { "s" };
                        ui.label(
                            RichText::new(format!(
                                "    {crates} crate{plural} in the jack frame at the rim. Leave the console and carry crates there (F) to change it."
                            ))
                            .small()
                            .color(DIM),
                        );
                        continue;
                    }
                    ui.horizontal(|ui| {
                        ui.label(format!("{}:", spec.label));
                        ui.add(egui::TextEdit::singleline(&mut ui_state.drafts[puzzle][i]).hint_text(spec.example).desired_width(170.0));
                    });
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
                    PuzzleKind::Rail => Some("Numbers need units: 12 m, not 12. Notebook names work: d1"),
                    _ if specs.iter().all(|s| s.source == InputSource::Placed) => None,
                    _ => Some("Numbers need units. Notebook names and functions work: (d1 + d2) / 2, sin(30 deg)"),
                };
                if let Some(hint) = hint {
                    ui.label(RichText::new(hint).small().color(DIM));
                }

                let charges = game.charges_left(puzzle);
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let ready = !sim.busy() && charges > 0;
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
                    ui.colored_label(WARN, format!("Not launched (no charge used). {refusal}"));
                }
                if sim.testing() == Some(puzzle) {
                    ui.label(RichText::new("Test running…").italics());
                }

                if let Some(outcome) = &ui_state.results[puzzle] {
                    ui.separator();
                    result_section(ui, &instance, outcome);
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
                        ui.label(format!("The {} now stands on the shipyard floor.", game.chapter.part.to_lowercase()));
                    }
                }
                if progress.attempts.len() > 1 {
                    ui.collapsing(format!("All tests ({})", progress.attempts.len()), |ui| {
                        for (n, attempt) in progress.attempts.iter().enumerate() {
                            let values: Vec<String> = attempt.values.iter().map(|v| v.quantity.display_in(&v.unit)).collect();
                            let colour = if attempt.pass { GOOD } else { DIM };
                            ui.colored_label(colour, format!("{}. {} — {}", n + 1, values.join(", "), attempt.headline));
                        }
                    });
                }
            });
        });
}

/// The ledge launcher's rating plate: its one speed, which can be noted like a ruler reading.
fn rating_plate(ui: &mut egui::Ui, game: &mut Game) {
    let (low, high) = (Quantity::speed(game.layout.canyon_speed_min), Quantity::speed(game.layout.canyon_speed_max));
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("Rating plate: the damaged dial turns only from {} to {}.", low.display(), high.display())).strong());
        let noted = game.save.notebook.entries.iter().any(|e| e.source == "plate");
        if ui.add_enabled(!noted, egui::Button::new("Note")).on_hover_text("Write both ends in the notebook, as vmin and vmax").clicked() {
            note_plate(game);
        }
    });
}

/// Write the ends of the ledge launcher's speed band into the notebook, as the plate's Note button does.
pub fn note_plate(game: &mut Game) {
    let (low, high) = (Quantity::speed(game.layout.canyon_speed_min), Quantity::speed(game.layout.canyon_speed_max));
    game.save.notebook.set("vmin", "lowest speed of the ledge launcher's dial", low, "plate");
    game.save.notebook.set("vmax", "highest speed of the ledge launcher's dial", high, "plate");
    game.persist();
}

/// The station's free experiment: a guess first, then the world shows what happens.
fn experiment_section(ui: &mut egui::Ui, game: &Game, ui_state: &mut Ui, sim: &mut Sim, puzzle: usize, demo: &DemoDef) {
    let id = &game.def(puzzle).id;
    let watched = game.save.demo_watched(id);
    let running = sim.demo() == Some(puzzle);
    ui.label(RichText::new("Try it first").strong());
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
            ui.label(RichText::new("Free, as often as you like.").small().color(DIM));
        }
    });
    if watched && !running {
        ui.label(RichText::new(&demo.reveal).color(Color32::from_rgb(200, 225, 240)));
    }
    if game.def(puzzle).kind == PuzzleKind::Gravity {
        // Where the light gate is, and the last reading: a pair from the gate, or the whole drop.
        let gate = game.placed(puzzle);
        let built = game.instance(puzzle).built(gate).unwrap_or_default();
        let line = if gate > 0 { format!("{}.", capitalise(&built)) } else { format!("Light gate: none fitted, so {}.", built.trim_start_matches("no light gate on the drop tower: ")) };
        ui.label(RichText::new(line).color(if gate > 0 { Color32::WHITE } else { DIM }));
        let entries = &game.save.notebook.entries;
        let last = entries.iter().rposition(|e| e.source == "stopwatch" || (e.source == "light gate" && e.quantity.dim == physics_core::units::Dim::TIME));
        match last.map(|i| &entries[i]) {
            Some(time) if time.source == "light gate" => {
                let number = time.name.trim_start_matches('t');
                let drop = game.save.notebook.get(&format!("h{number}"));
                let drop = drop.map_or(String::new(), |h| format!("{} = {}, ", h.name, h.quantity.display()));
                ui.label(format!("Light gate: {drop}{} = {}", time.name, time.quantity.display()))
            }
            Some(time) => ui.label(format!("Stopwatch: {} = {}", time.name, time.quantity.display())),
            None => ui.label(RichText::new("Stopwatch: no reading yet").color(DIM)),
        };
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
    if let Playback::Flight { projectile, flight } = &outcome.playback
        && let Some(profile) = instance.profile()
    {
        // Side view: terrain, target, and the path that was flown.
        let layout = &instance.layout;
        let (target, half) = match instance.kind {
            PuzzleKind::Canyon => (layout.platform_centre, layout.platform_half),
            _ => (layout.pad_centre, layout.pad_half),
        };
        let x_min = -3.0;
        let x_max = (target + half + 6.0).max(flight.x + 3.0).min(profile.x_end());
        let (_, _, apex) = projectile.apex();
        let y_min = if instance.kind == PuzzleKind::Canyon { -(layout.canyon_depth as f64) - 1.0 } else { -2.0 };
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
}

fn ruler_panel(ctx: &egui::Context, game: &mut Game, ruler: &mut Ruler) {
    let mut open = true;
    egui::Window::new("Ruler")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::LEFT_BOTTOM, [10.0, -10.0])
        .show(ctx, |ui| {
            let hover = ruler.hover.as_ref().map(|h| h.label.as_str());
            match (&ruler.first, &ruler.second) {
                (None, _) => {
                    ui.label("Click the first point.");
                    ui.label(RichText::new(hover.unwrap_or("Yellow pins mark surveyed points.")).color(DIM));
                }
                (Some(first), None) => {
                    ui.label(format!("From: {}", first.label));
                    ui.label(RichText::new(format!("To: {}", hover.unwrap_or("click the second point"))).color(DIM));
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
                for (word, prefix, value) in [("Horizontal", "d", m.horizontal), ("Vertical", "h", m.vertical), ("Direct", "s", m.direct)] {
                    ui.label(word);
                    ui.label(RichText::new(Quantity::metres(value as f64).display()).strong());
                    if ui.button(format!("Note as {prefix}…")).clicked() {
                        saved = Some((prefix, word, value));
                    }
                    ui.end_row();
                }
            });
            if let Some((prefix, word, value)) = saved {
                let label = format!("{} distance, {between}", word.to_lowercase());
                game.save.notebook.add_numbered(prefix, &label, Quantity::metres(value as f64), "ruler");
                game.persist();
            }
            ui.label(RichText::new("R measures again · Esc puts the ruler away").small().color(DIM));
        });
    if !open {
        ruler.put_away();
    }
}

fn notebook_body(ui: &mut egui::Ui, game: &mut Game, ui_state: &mut Ui, room: f32) {
    if game.save.notebook.entries.is_empty() {
        ui.label(RichText::new("Nothing measured yet. Readings from the ruler and the stopwatch are kept here.").color(DIM));
    }
    let mut remove = None;
    // The calculator under the entries needs about this much.
    let entries_room = (room - 90.0).max(30.0);
    egui::ScrollArea::vertical().id_salt("notebook-entries").max_height(entries_room).show(ui, |ui| {
        egui::Grid::new("notebook").num_columns(4).striped(true).show(ui, |ui| {
            for entry in &game.save.notebook.entries {
                ui.label(RichText::new(&entry.name).strong().monospace());
                ui.label(entry.quantity.display());
                ui.label(RichText::new(&entry.label).small().color(DIM)).on_hover_text(format!("Source: {}", entry.source));
                if ui.small_button("×").on_hover_text("Remove this entry").clicked() {
                    remove = Some(entry.name.clone());
                }
                ui.end_row();
            }
        });
    });
    if let Some(name) = remove {
        game.save.notebook.remove(&name);
        game.persist();
    }

    ui.separator();
    ui.label(RichText::new("Calculator").strong());
    ui.add(egui::TextEdit::singleline(&mut ui_state.calculator).hint_text("(t1 + t2) / 2   or   tavg = (t1 + t2) / 2").desired_width(360.0));
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
                        if ui.button(format!("Note as {name}")).clicked() {
                            game.save.notebook.set(&name, &expression, q, "calculated");
                            game.persist();
                            ui_state.calculator.clear();
                        }
                    }
                    Some(name) => {
                        ui.colored_label(WARN, format!("'{name}' cannot be a notebook name: use letters and digits, starting with a letter, and not a unit."));
                    }
                    None => {
                        ui.label(RichText::new("Write  name = expression  to keep a result.").small().color(DIM));
                    }
                }
            }
            Err(e) => {
                ui.colored_label(BAD, e.to_string());
            }
        }
    }
}

/// A notebook name must read as a variable and must not hide a unit or a function.
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let starts_well = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    let reserved = ["sqrt", "sin", "cos", "tan", "asin", "acos", "atan", "ln", "exp", "abs"];
    starts_well && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') && !reserved.contains(&name) && (name == "g" || lookup_unit(name).is_none())
}

fn handbook_body(ui: &mut egui::Ui, game: &Game, ui_state: &mut Ui, room: f32) {
    egui::ScrollArea::vertical().id_salt("handbook").max_height(room).show(ui, |ui| {
        ui.label(RichText::new("Pages").strong());
        ui.label(RichText::new("A page is written when you see what it describes happen in the world.").small().color(DIM));
        let focus = ui_state.handbook_focus.take();
        // A page is listed once a puzzle that uses it is open; it can be read once found.
        let mut shown: Vec<&str> = Vec::new();
        for (i, puzzle) in game.chapter.puzzles.iter().enumerate() {
            if !game.unlocked(i) {
                continue;
            }
            for id in &puzzle.concepts {
                if shown.contains(&id.as_str()) {
                    continue;
                }
                shown.push(id);
                let Some(concept) = game.chapter.concept(id) else { continue };
                if !game.page_found(id) {
                    ui.label(RichText::new(format!("{} — not found yet", concept.title)).color(DIM));
                    ui.label(RichText::new(format!("    {}", concept.hint)).small().italics().color(DIM));
                    continue;
                }
                let new = ui_state.unread_pages.contains(&concept.id);
                let title = if new { RichText::new(format!("{}   · new", concept.title)).color(GOOD) } else { RichText::new(&concept.title) };
                let focused = focus.as_deref() == Some(id.as_str());
                let mut header = egui::CollapsingHeader::new(title).id_salt(("page", id));
                if focused {
                    header = header.open(Some(true));
                }
                let shown_page = header.show(ui, |ui| {
                    ui.label(RichText::new(&concept.card).monospace());
                    if let Some(at) = game.chapter.puzzle(concept.found.puzzle()) {
                        ui.label(RichText::new(format!("Found at: {}", at.title)).small().color(DIM));
                    }
                });
                if focused {
                    shown_page.header_response.scroll_to_me(Some(egui::Align::TOP));
                }
            }
        }
        ui.separator();
        ui.label(RichText::new("Materials (one block is one cubic metre)").strong());
        egui::Grid::new("materials").num_columns(5).striped(true).show(ui, |ui| {
            for heading in ["", "density\nkg/m³", "specific heat\nJ/(kg K)", "conductivity\nW/(m K)", "resistivity\nohm m"] {
                ui.label(RichText::new(heading).small().color(DIM));
            }
            ui.end_row();
            for material in Material::ALL {
                let p = material.properties();
                ui.label(p.name);
                ui.label(sig(p.density, 3));
                ui.label(sig(p.specific_heat, 3));
                ui.label(sig(p.thermal_conductivity, 2));
                ui.label(if p.resistivity.is_finite() { format!("{:.1e}", p.resistivity) } else { "insulator".into() });
                ui.end_row();
            }
        });
    });
}

/// The conversation with Lumen: drawn by the companion inside the panel that grows out of its eye.
pub fn tutor_body(ui: &mut egui::Ui, game: &Game, ui_state: &mut Ui, lumen: &mut Lumen, landmarks: &Landmarks, room: f32) {
    let puzzle = ui_state.focus(game);
    ui.set_width(430.0);
    if !lumen.tutor.is_online() {
        ui.colored_label(WARN, "Lumen is offline.");
        ui.label(lumen.tutor.status());
        ui.label(RichText::new("To connect a language model, add a [tutor] section to settings.toml (see docs/tutor.md). The handbook (H) works without it.").color(DIM));
        return;
    }
    ui.label(RichText::new(format!("Looking at: {} · {}", game.def(puzzle).title, lumen.tutor.status())).small().color(DIM));
    ui.label(RichText::new("Lumen can see this puzzle and explain. It cannot act in the world.").small().color(DIM));
    let conversation = &lumen.conversations[puzzle];
    let mut send = false;
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
        let clicked = ui.add_enabled(!conversation.is_waiting(), egui::Button::new("Ask")).clicked();
        send = (entered || clicked) && !conversation.is_waiting();
    });
    if send && !ui_state.chat.trim().is_empty() {
        let text = std::mem::take(&mut ui_state.chat);
        lumen.ask(game, ui_state, landmarks, puzzle, &text);
    }
}
