//! `GAME_SCRIPT=prefill`: you play by hand while the bookkeeping is done for you. Development only.
//!
//! On taking over from the menu, the ruler readings the tour takes go into the notebook (`d1` along
//! the rail; `d2`, `d3`, `d4` at the haul track; `h1` up the drop tower; `d5`, `h2` across the canyon;
//! `d6` to the ridge's landing circle), and the ledge launcher's rating plate is noted. Each station's
//! blueprint is then kept filled with the tour's formula, built from the readings the notebook holds:
//! the crawler's speed waits for its stopwatch time, `g` for a light gate reading, the canyon's speed
//! for the stack in the jack frame and the calibrated `g`. A blueprint you type into is yours and is
//! left alone; clear it to get the formula back. While a station's panel is open, the calculator
//! offers the tour's working there in the same way.
//!
//! Everything in the world is still yours to do: walking, carrying the cart's block, the light gate and
//! the canyon's blocks, running the experiments, committing.

use crate::sim::CRAWLER_TIME;
use crate::island::Sites;
use crate::state::{Game, Mode, PUZZLES, Ui};
use bevy::prelude::*;
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

/// The ruler readings the tour takes: name, the two pins (as indices into `Sites::pins`), and whether
/// the reading is the vertical separation rather than the horizontal one.
const READINGS: [(&str, usize, usize, bool); 8] = [
    ("d1", 10, 11, false),
    ("d2", HAUL_PINS, HAUL_PINS + 1, false),
    ("d3", HAUL_PINS, HAUL_PINS + 2, false),
    ("d4", HAUL_PINS + 3, HAUL_PINS + 4, false),
    ("h1", 0, 1, true),
    ("d5", 4, 5, false),
    ("h2", 4, 5, true),
    ("d6", 6, 9, false),
];

/// The haul track's first pin: the cart's front coupler, then the crawler, the marker and the posts.
const HAUL_PINS: usize = 13;

fn note_readings(mut game: ResMut<Game>, sites: Res<Sites>) {
    if write_readings(&mut game, &sites) {
        game.persist();
    }
}

/// Note every reading the notebook does not hold yet, as the ruler's buttons would, and the rating
/// plate. Whether anything was written.
fn write_readings(game: &mut Game, sites: &Sites) -> bool {
    let mut wrote = false;
    for (name, a, b, vertical) in READINGS {
        if game.save.notebook.get(name).is_some() {
            continue;
        }
        let (a, b) = (&sites.pins[a], &sites.pins[b]);
        let d = b.position - a.position;
        let (word, value) = if vertical { ("vertical", d.y.abs()) } else { ("horizontal", d.with_y(0.0).length()) };
        let label = format!("{word} distance, {} to {}", a.label, b.label);
        game.save.notebook.set(name, &label, Quantity::metres(value as f64), "ruler");
        wrote = true;
    }
    if !game.save.notebook.entries.iter().any(|e| e.source == "plate") {
        crate::ui::note_plate(game);
        wrote = true;
    }
    wrote
}

fn fill(game: Res<Game>, mut ui: ResMut<Ui>, mut prefill: ResMut<Prefill>) {
    let ui = &mut *ui;
    for puzzle in 0..PUZZLES {
        let Some(wanted) = formula(&game, puzzle) else { continue };
        let draft = &ui.drafts[puzzle];
        let untouched = draft.iter().all(|d| d.trim().is_empty()) || *draft == prefill.drafts[puzzle];
        if untouched && *draft != wanted {
            ui.drafts[puzzle] = wanted.clone();
            prefill.drafts[puzzle] = wanted;
        }
    }
    let Some(wanted) = ui.station.and_then(|puzzle| working(&game, puzzle)) else { return };
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
    let has = |name: &str| notebook.get(name).is_some();
    match puzzle {
        0 => has("d1").then(|| vec!["d1".into()]),
        1 => {
            let t = crawler_time(game)?;
            (has("d2") && has("d3") && has("d4")).then(|| vec![format!("(d4/{t})*d3/(d3 - d2)")])
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
            (has("d5") && has("h2") && has("g")).then(|| vec![String::new(), format!("d5/sqrt(2*(h2 + {stack} m)/g)")])
        }
        4 => {
            let (min, max) = game.layout.ridge_angle_window();
            let angle = ((min + max) / 2.0).to_degrees().round();
            (has("d6") && has("g")).then(|| vec![format!("{angle} deg"), format!("sqrt(g*d6/sin(2*{angle} deg))")])
        }
        _ => None,
    }
}

/// What the tour works out in the calculator at a station, once the readings it uses are noted.
fn working(game: &Game, puzzle: usize) -> Option<String> {
    let has = |name: &str| game.save.notebook.get(name).is_some();
    match puzzle {
        1 => crawler_time(game).filter(|_| has("d4")).map(|t| format!("u = d4/{t}")),
        3 => ["g", "d5", "vmax", "h2"].into_iter().all(has).then(|| "hmin = g*d5^2/(2*vmax^2) - h2".into()),
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

    #[test]
    fn the_prefilled_formulas_solve_every_station() {
        for seed in [3, 7, 21] {
            let mut game = Game::for_test(seed);
            let (_, sites) = crate::island::build(&game.layout);
            let mut ui = Ui::default();
            assert!(write_readings(&mut game, &sites));
            assert!(!write_readings(&mut game, &sites), "readings already noted are not noted again");

            let cart = game.def(0).id.clone();
            game.save.placed.insert(cart, 1);
            commit(&mut game, &mut ui, 0);

            assert!(formula(&game, 1).is_none(), "the chase waits for the stopwatch");
            note_crawler(&mut game);
            commit(&mut game, &mut ui, 1);

            assert!(formula(&game, 2).is_none(), "gravity waits for the light gate");
            note_reading(&mut game, 0);
            note_reading(&mut game, 3);
            let long = game.layout.tower_h - 2;
            note_reading(&mut game, long);
            game.save.placed.insert(game.def(2).id.clone(), long);
            commit(&mut game, &mut ui, 2);

            let stack = *game.layout.canyon_stacks().start();
            game.save.placed.insert(game.def(3).id.clone(), stack);
            commit(&mut game, &mut ui, 3);
            commit(&mut game, &mut ui, 4);
        }
    }
}
