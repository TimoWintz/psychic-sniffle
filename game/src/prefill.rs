//! `GAME_SCRIPT=prefill`: you play by hand while the bookkeeping is done for you. Development only.
//!
//! On taking over from the menu, the ruler readings the tour takes go into the notebook (on a fresh
//! save: `d1` along the rail; `d2`, `d3`, `d4` at the haul track; `h1` up the drop tower; `d5`, `h2`
//! across the canyon; `d6` to the ridge's landing circle), and the ledge launcher's rating plate is
//! noted. A reading is known by its label, as the ruler writes it, so one the notebook already holds
//! is not noted again, and a name the notebook already uses for something else is not taken. Each
//! station's blueprint is then kept filled with the tour's formula, built from the readings the
//! notebook holds: the crawler's speed waits for its stopwatch time, `g` for a light gate reading,
//! the canyon's speed for the stack in the jack frame and the calibrated `g`. A blueprint you type
//! into is yours and is left alone; clear it to get the formula back. While a station's panel is
//! open, the calculator offers the tour's working there in the same way, until its result is noted.
//!
//! Everything in the world is still yours to do: walking, carrying the cart's block, the light gate and
//! the canyon's blocks, running the experiments, committing.

use crate::island::Sites;
use crate::sim::CRAWLER_TIME;
use crate::state::{Game, Mode, PUZZLES, Ui};
use bevy::prelude::*;
use physics_core::notebook::Notebook;
use physics_core::units::{Dim, Quantity};

pub struct PrefillPlugin;

impl Plugin for PrefillPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Prefill>()
            .add_systems(OnEnter(Mode::Playing), note_readings)
            .add_systems(Update, fill.run_if(in_state(Mode::Playing)));
    }
}

/// What was last written into each blueprint and into the calculator, to tell it from the player's
/// own typing.
#[derive(Resource, Default)]
struct Prefill {
    drafts: [Vec<String>; PUZZLES],
    calculator: String,
}

/// The ruler readings the tour takes: the name it gets on a fresh save, the two survey pins (by
/// label), and whether the reading is the vertical separation rather than the horizontal one.
const READINGS: [(&str, &str, &str, bool); 8] = [
    ("d1", "Rail cart: starting point", "Rail marker: centre", false),
    ("d2", "Haul cart: front coupler", "Crawler: rear coupler", false),
    ("d3", "Haul cart: front coupler", "Transfer marker: centre", false),
    ("d4", "Timing post A", "Timing post B", false),
    ("h1", "Drop tower: release point", "Drop tower: landing plate", true),
    ("d5", "Ledge launcher: release point", "Platform: centre", false),
    ("h2", "Ledge launcher: release point", "Platform: centre", true),
    ("d6", "Tilting launcher: release point", "Landing circle: centre", false),
];

/// The notebook label of a reading, as the ruler's Note buttons write it.
fn label(a: &str, b: &str, vertical: bool) -> String {
    format!("{} distance, {a} to {b}", if vertical { "vertical" } else { "horizontal" })
}

/// The name under which the notebook holds one of the tour's readings (by its fresh-save name).
fn reading(notebook: &Notebook, role: &str) -> Option<String> {
    let (_, a, b, vertical) = READINGS.iter().find(|r| r.0 == role).expect("a reading of the tour");
    let label = label(a, b, *vertical);
    notebook.entries.iter().find(|e| e.label == label).map(|e| e.name.clone())
}

fn note_readings(mut game: ResMut<Game>, sites: Res<Sites>) {
    if write_readings(&mut game, &sites) {
        game.persist();
    }
}

/// Note every reading the notebook does not hold yet, as the ruler's buttons would, and the rating
/// plate. Whether anything was written; the caller persists.
fn write_readings(game: &mut Game, sites: &Sites) -> bool {
    let mut wrote = false;
    for (role, a, b, vertical) in READINGS {
        let label = label(a, b, vertical);
        if game.save.notebook.entries.iter().any(|e| e.label == label) {
            continue;
        }
        let pin = |name: &str| sites.pins.iter().position(|p| p.label == name).unwrap_or_else(|| panic!("no survey pin '{name}'"));
        let (a, b) = (pin(a), pin(b));
        let mut from = sites.pins[a].position;
        // The launcher's pin rises with the blocks in the jack frame; the tour measures from the
        // bare ledge, and the blueprint adds the stack.
        if a == sites.canyon_release_pin {
            from.y = sites.canyon_well.y as f32;
        }
        let d = sites.pins[b].position - from;
        let value = if vertical { d.y.abs() } else { d.with_y(0.0).length() };
        game.save.notebook.add_numbered(&role[..1], &label, Quantity::metres(value as f64), "ruler");
        wrote = true;
    }
    if !game.save.notebook.entries.iter().any(|e| e.source == "plate") {
        crate::ui::write_plate(game);
        wrote = true;
    }
    wrote
}

fn fill(game: Res<Game>, mut ui: ResMut<Ui>, mut prefill: ResMut<Prefill>) {
    refresh(&game, &mut ui, &mut prefill);
}

/// Write the tour's formulas into the blueprints, and its working into the calculator, wherever
/// what is there is blank or was written here.
fn refresh(game: &Game, ui: &mut Ui, prefill: &mut Prefill) {
    for puzzle in 0..PUZZLES {
        let Some(wanted) = formula(game, puzzle) else { continue };
        let draft = &ui.drafts[puzzle];
        let untouched = draft.iter().all(|d| d.trim().is_empty()) || *draft == prefill.drafts[puzzle];
        if untouched && *draft != wanted {
            ui.drafts[puzzle] = wanted.clone();
            prefill.drafts[puzzle] = wanted;
        }
    }
    let Some(wanted) = ui.station.and_then(|puzzle| working(game, puzzle)) else { return };
    let untouched = ui.calculator.trim().is_empty() || ui.calculator == prefill.calculator;
    if untouched && ui.calculator != wanted {
        ui.calculator = wanted.clone();
        prefill.calculator = wanted;
    }
}

/// The blueprint the tour commits at a station, one text per input, from what the notebook holds; or
/// nothing while a reading it needs is missing.
fn formula(game: &Game, puzzle: usize) -> Option<Vec<String>> {
    let notebook = &game.save.notebook;
    let name = |role: &str| reading(notebook, role);
    let has = |name: &str| notebook.get(name).is_some();
    match puzzle {
        0 => Some(vec![name("d1")?]),
        1 => {
            let t = crawler_time(game)?;
            let (d2, d3, d4) = (name("d2")?, name("d3")?, name("d4")?);
            Some(vec![format!("({d4}/{t})*{d3}/({d3} - {d2})")])
        }
        2 => {
            // The light gate's reading over the longest drop; the stopwatch's own is too rough.
            let (h, t) = notebook
                .entries
                .iter()
                .filter(|e| e.source == "light gate" && e.quantity.dim == Dim::TIME)
                .filter_map(|t| {
                    let h = format!("h{}", t.name.trim_start_matches('t'));
                    notebook.get(&h).map(|drop| (drop.quantity.value, h, t.name.clone()))
                })
                .max_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, h, t)| (h, t))?;
            Some(vec![format!("2*{h}/{t}^2")])
        }
        3 => {
            // For the stack in the jack frame, once it is one the dial can reach; until then, the
            // lowest such stack, which is the one to build.
            let stacks = game.layout.canyon_stacks();
            let placed = game.placed(3);
            let stack = if stacks.contains(&placed) { placed } else { *stacks.start() };
            let (d5, h2) = (name("d5")?, name("h2")?);
            has("g").then(|| vec![String::new(), format!("{d5}/sqrt(2*({h2} + {stack} m)/g)")])
        }
        4 => {
            let (min, max) = game.layout.ridge_angle_window();
            let angle = ((min + max) / 2.0).to_degrees().round();
            let d6 = name("d6")?;
            has("g").then(|| vec![format!("{angle} deg"), format!("sqrt(g*{d6}/sin(2*{angle} deg))")])
        }
        _ => None,
    }
}

/// What the tour works out in the calculator at a station, once the readings it uses are noted and
/// until its result is.
fn working(game: &Game, puzzle: usize) -> Option<String> {
    let notebook = &game.save.notebook;
    let name = |role: &str| reading(notebook, role);
    let has = |name: &str| notebook.get(name).is_some();
    match puzzle {
        1 if !has("u") => Some(format!("u = {}/{}", name("d4")?, crawler_time(game)?)),
        3 if !has("hmin") => {
            let (d5, h2) = (name("d5")?, name("h2")?);
            (has("g") && has("vmax")).then(|| format!("hmin = g*{d5}^2/(2*vmax^2) - {h2}"))
        }
        _ => None,
    }
}

/// The name of the stopwatch's latest time of the crawler between the posts.
fn crawler_time(game: &Game) -> Option<String> {
    game.save.notebook.entries.iter().rev().find(|e| e.label == CRAWLER_TIME).map(|e| e.name.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{judge_commit, note_crawler, note_reading};

    /// Commit the prefilled blueprint at a station as the player would, and record the result.
    fn commit(game: &mut Game, ui: &mut Ui, puzzle: usize) {
        ui.drafts[puzzle] = formula(game, puzzle).unwrap_or_else(|| panic!("station {puzzle} has no formula yet"));
        let (outcome, values) = judge_commit(game, ui, puzzle).unwrap_or_else(|| panic!("station {puzzle} refused: {:?}", ui.refusal));
        assert!(outcome.pass, "station {puzzle} missed: {}", outcome.detail);
        if puzzle == 2 {
            game.save.notebook.set("g", "gravitational acceleration (your calibration)", values[0].quantity, "calibrated");
        }
        let id = game.def(puzzle).id.clone();
        game.save.record(&id, values, &outcome);
    }

    /// Play the chapter through with the prefilled formulas, from whatever the notebook holds.
    fn play_through(game: &mut Game, sites: &Sites) {
        let mut ui = Ui::default();
        assert!(write_readings(game, sites));
        assert!(!write_readings(game, sites), "readings already noted are not noted again");

        let cart = game.def(0).id.clone();
        game.save.placed.insert(cart, 1);
        commit(game, &mut ui, 0);

        assert!(formula(game, 1).is_none(), "the chase waits for the stopwatch");
        note_crawler(game);
        commit(game, &mut ui, 1);

        assert!(formula(game, 2).is_none(), "gravity waits for the light gate");
        note_reading(game, 0);
        note_reading(game, 3);
        let long = game.layout.tower_h - 2;
        note_reading(game, long);
        game.save.placed.insert(game.def(2).id.clone(), long);
        commit(game, &mut ui, 2);

        let stack = *game.layout.canyon_stacks().start();
        game.save.placed.insert(game.def(3).id.clone(), stack);
        commit(game, &mut ui, 3);
        commit(game, &mut ui, 4);
    }

    #[test]
    fn the_prefilled_formulas_solve_every_station() {
        for seed in [3, 7, 21] {
            let mut game = Game::for_test(seed);
            let (_, sites) = crate::island::build(&game.layout);
            play_through(&mut game, &sites);
            for (role, ..) in READINGS {
                assert_eq!(reading(&game.save.notebook, role).as_deref(), Some(role), "seed {seed}: a fresh save names {role} as the tour does");
            }
        }
    }

    #[test]
    fn a_resumed_save_keeps_its_names_and_its_stack() {
        for seed in [3, 7, 21] {
            let mut game = Game::for_test(seed);
            let (_, mut sites) = crate::island::build(&game.layout);
            // The player noted the canyon's width before anything else: it holds d2 and h2.
            game.save.notebook.set("d2", "horizontal distance, somewhere else", Quantity::metres(99.0), "ruler");
            game.save.notebook.set("h2", "drop from the release point to the light gate", Quantity::metres(1.0), "light gate");
            // And two blocks stand in the jack frame, raising the launcher's pin.
            let pin = sites.canyon_release_pin;
            sites.pins[pin].position.y += 2.0;
            play_through(&mut game, &sites);
            assert_eq!(game.save.notebook.get("d2").expect("kept").quantity.value, 99.0, "seed {seed}: the player's d2 is kept");
            let h = reading(&game.save.notebook, "h2").expect("noted");
            assert_ne!(h, "h2", "seed {seed}");
            let bare = (sites.canyon_well.y as f32 - sites.pins[pin + 1].position.y).abs();
            assert!((game.save.notebook.get(&h).expect("noted").quantity.value - bare as f64).abs() < 1e-6, "seed {seed}: the canyon is measured from the bare ledge");
        }
    }

    #[test]
    fn typing_is_left_alone_and_clearing_brings_the_formula_back() {
        let mut game = Game::for_test(7);
        let (_, sites) = crate::island::build(&game.layout);
        let (mut ui, mut prefill) = (Ui::default(), Prefill::default());
        write_readings(&mut game, &sites);
        refresh(&game, &mut ui, &mut prefill);
        assert_eq!(ui.drafts[0], vec!["d1".to_string()]);

        ui.drafts[0] = vec!["d1 + 1 m".into()];
        refresh(&game, &mut ui, &mut prefill);
        assert_eq!(ui.drafts[0], vec!["d1 + 1 m".to_string()], "the player's typing is kept");
        ui.drafts[0] = vec![String::new()];
        refresh(&game, &mut ui, &mut prefill);
        assert_eq!(ui.drafts[0], vec!["d1".to_string()], "a cleared blueprint gets the formula back");

        // The canyon's blueprint follows the stack in the jack frame.
        game.save.notebook.set("g", "gravitational acceleration (your calibration)", Quantity::metres(9.8), "calibrated");
        let stacks = game.layout.canyon_stacks();
        refresh(&game, &mut ui, &mut prefill);
        assert!(ui.drafts[3][1].contains(&format!("+ {} m", stacks.start())), "{:?}", ui.drafts[3]);
        game.save.placed.insert(game.def(3).id.clone(), *stacks.end());
        refresh(&game, &mut ui, &mut prefill);
        assert!(ui.drafts[3][1].contains(&format!("+ {} m", stacks.end())), "{:?}", ui.drafts[3]);

        // The calculator offers the working until its result is noted, then stays clear.
        note_crawler(&mut game);
        ui.station = Some(1);
        refresh(&game, &mut ui, &mut prefill);
        assert_eq!(ui.calculator, "u = d4/t1");
        game.save.notebook.set("u", "d4/t1", Quantity::metres(1.0), "calculated");
        ui.calculator.clear();
        refresh(&game, &mut ui, &mut prefill);
        assert!(ui.calculator.is_empty(), "a noted result is not offered again");
    }
}
