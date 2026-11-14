//! Interface panels: heads-up display, station, ruler readout, notebook and calculator, handbook,
//! and the conversation with Lumen.

use crate::companion::Landmarks;
use crate::ruler::Ruler;
use crate::sim::{Request, Sim};
use crate::state::{Game, Ui};
use crate::tutor_bridge::Lumen;
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use egui::{Align2, Color32, RichText};
use physics_core::ballistics::FlightEnd;
use physics_core::expr::evaluate;
use physics_core::materials::Material;
use physics_core::progress::Quality;
use physics_core::puzzle::{Instance, Outcome, Playback, PuzzleKind};
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
            .add_systems(Update, hotkeys)
            .add_systems(EguiPrimaryContextPass, draw);
    }
}

fn hotkeys(keys: Res<ButtonInput<KeyCode>>, wants: Res<EguiWantsInput>, mut ui: ResMut<Ui>, mut ruler: ResMut<Ruler>) {
    if wants.wants_any_keyboard_input() {
        return;
    }
    if keys.just_pressed(KeyCode::KeyN) {
        ui.show_notebook = !ui.show_notebook;
    }
    if keys.just_pressed(KeyCode::KeyH) {
        ui.show_handbook = !ui.show_handbook;
    }
    if keys.just_pressed(KeyCode::KeyT) {
        ui.show_tutor = !ui.show_tutor;
    }
    if keys.just_pressed(KeyCode::KeyR) {
        ruler.toggle();
    }
}

#[allow(clippy::too_many_arguments)]
pub fn draw(
    mut contexts: EguiContexts,
    mut game: ResMut<Game>,
    mut ui_state: ResMut<Ui>,
    mut sim: ResMut<Sim>,
    mut ruler: ResMut<Ruler>,
    mut lumen: ResMut<Lumen>,
    landmarks: Res<Landmarks>,
    mut styled: Local<bool>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    if !*styled {
        *styled = true;
        ctx.set_zoom_factor(1.15);
    }
    // Reborrow once so the panels can borrow fields separately.
    let (game, ui_state, sim, ruler, lumen) = (&mut *game, &mut *ui_state, &mut *sim, &mut *ruler, &mut *lumen);

    hud(&ctx, game, ui_state, ruler);
    if let Some(puzzle) = ui_state.station {
        station_panel(&ctx, game, ui_state, sim, puzzle);
    }
    if ruler.active {
        ruler_panel(&ctx, game, ruler);
    }
    if ui_state.show_notebook {
        notebook_panel(&ctx, game, ui_state);
    }
    if ui_state.show_handbook {
        handbook_panel(&ctx, game, ui_state);
    }
    if ui_state.show_tutor {
        tutor_panel(&ctx, game, ui_state, lumen, &landmarks);
    }
    Ok(())
}

fn hud(ctx: &egui::Context, game: &Game, ui_state: &Ui, ruler: &Ruler) {
    egui::Window::new("hud")
        .title_bar(false)
        .resizable(false)
        .anchor(Align2::LEFT_TOP, [10.0, 10.0])
        .show(ctx, |ui| {
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

    let prompt = if ruler.active {
        Some("Ruler on: left-click two points. R puts it away.".to_string())
    } else {
        match (ui_state.near_station, ui_state.station) {
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
            ui.label(
                RichText::new("WASD move · Shift run · Space jump · right-drag or arrows look · scroll zoom · E station · R ruler · N notebook · H handbook · T or click Lumen")
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
            egui::ScrollArea::vertical().max_height(ctx.content_rect().height() - 80.0).show(ui, |ui| {
                ui.label(&def.brief);
                if def.kind == PuzzleKind::Ridge {
                    ui.label(RichText::new(format!("Launcher rating: {} maximum.", Quantity::speed(game.layout.v_max).display())).strong());
                }
                ui.add_space(6.0);
                ui.label(RichText::new("Investigate").strong());
                ui.label(RichText::new(&def.investigate).color(DIM));

                if def.kind == PuzzleKind::Gravity {
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!sim.busy(), egui::Button::new("Release the test mass")).clicked() {
                            sim.request = Some(Request::ReleaseTowerMass);
                        }
                        let readings: Vec<String> = game
                            .save
                            .notebook
                            .entries
                            .iter()
                            .filter(|e| e.source == "stopwatch")
                            .map(|e| format!("{} = {}", e.name, e.quantity.display()))
                            .collect();
                        match readings.last() {
                            Some(last) => ui.label(format!("Stopwatch: {last}")),
                            None => ui.label(RichText::new("Stopwatch: no reading yet").color(DIM)),
                        };
                    });
                }

                ui.add_space(6.0);
                ui.separator();
                ui.label(RichText::new("Blueprint").strong());
                for (i, spec) in specs.iter().enumerate() {
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
                let hint = if def.kind == PuzzleKind::Rail {
                    "Numbers need units: 12 m, not 12. Notebook names work: d1"
                } else {
                    "Numbers need units. Notebook names work: sqrt(2*h1/g)"
                };
                ui.label(RichText::new(hint).small().color(DIM));

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
                if sim.busy() {
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
    egui::Window::new("Ruler")
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
        });
}

fn notebook_panel(ctx: &egui::Context, game: &mut Game, ui_state: &mut Ui) {
    let mut open = true;
    egui::Window::new("Notebook and calculator")
        .open(&mut open)
        .default_pos([12.0, 96.0])
        .resizable(false)
        .show(ctx, |ui| {
            if game.save.notebook.entries.is_empty() {
                ui.label(RichText::new("Nothing measured yet. Readings from the ruler and the stopwatch are kept here.").color(DIM));
            }
            let mut remove = None;
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
            if let Some(name) = remove {
                game.save.notebook.remove(&name);
                game.persist();
            }

            ui.separator();
            ui.label(RichText::new("Calculator").strong());
            ui.add(egui::TextEdit::singleline(&mut ui_state.calculator).hint_text("2*h1/t1^2   or   g1 = 2*h1/t1^2").desired_width(360.0));
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
        });
    ui_state.show_notebook = open;
}

/// A notebook name must read as a variable and must not hide a unit or a function.
fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let starts_well = chars.next().is_some_and(|c| c.is_ascii_alphabetic());
    let reserved = ["sqrt", "sin", "cos", "tan", "asin", "acos", "atan", "ln", "exp", "abs"];
    starts_well && chars.all(|c| c.is_ascii_alphanumeric() || c == '_') && !reserved.contains(&name) && (name == "g" || lookup_unit(name).is_none())
}

fn handbook_panel(ctx: &egui::Context, game: &Game, ui_state: &mut Ui) {
    let mut open = true;
    egui::Window::new("Handbook")
        .open(&mut open)
        .default_pos([300.0, 60.0])
        .resizable(false)
        .show(ctx, |ui| {
            ui.set_width(480.0);
            egui::ScrollArea::vertical().max_height(430.0).show(ui, |ui| {
                ui.label(RichText::new("Concepts").strong());
                // A concept card appears once a puzzle that uses it is open.
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
                        if let Some(concept) = game.chapter.concept(id) {
                            ui.collapsing(&concept.title, |ui| ui.label(RichText::new(&concept.card).monospace()));
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
        });
    ui_state.show_handbook = open;
}

fn tutor_panel(ctx: &egui::Context, game: &Game, ui_state: &mut Ui, lumen: &mut Lumen, landmarks: &Landmarks) {
    let puzzle = ui_state.focus(game);
    let mut open = true;
    let mut send = false;
    egui::Window::new("Lumen")
        .open(&mut open)
        .pivot(Align2::LEFT_BOTTOM)
        .default_pos([12.0, ctx.content_rect().bottom() - 160.0])
        .resizable(false)
        .show(ctx, |ui| {
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
            egui::ScrollArea::vertical().max_height(300.0).stick_to_bottom(true).show(ui, |ui| {
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
        });
    if send && !ui_state.chat.trim().is_empty() {
        let text = std::mem::take(&mut ui_state.chat);
        lumen.ask(game, ui_state, landmarks, puzzle, &text);
    }
    ui_state.show_tutor = open;
}
