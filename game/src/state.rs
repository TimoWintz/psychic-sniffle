//! Game state shared between systems: the chapter, the seeded layout, the save, and what the
//! interface is showing.

use bevy::prelude::*;
use physics_core::content::{Chapter, PuzzleDef, chapter_1_1};
use physics_core::progress::Save;
use physics_core::puzzle::{Ch1Layout, Instance, Outcome};
use std::path::PathBuf;

#[derive(Resource)]
pub struct Game {
    pub chapter: Chapter,
    pub layout: Ch1Layout,
    pub save: Save,
    pub save_path: PathBuf,
    /// Set if the save could not be read or written; shown in the interface.
    pub save_problem: Option<String>,
}

impl Game {
    /// Continue from the save file, or start a new game.
    ///
    /// `GAME_SAVE` overrides the save path and `GAME_SEED` the seed of a new game.
    pub fn load_or_new() -> Self {
        let save_path = PathBuf::from(std::env::var("GAME_SAVE").unwrap_or_else(|_| "saves/slot1.ron".into()));
        let mut save_problem = None;
        let loaded = match std::fs::read_to_string(&save_path) {
            Ok(text) => match Save::from_ron(&text) {
                Ok(save) => Some(save),
                Err(e) => {
                    save_problem = Some(format!("{} could not be read ({e}); started a new game.", save_path.display()));
                    None
                }
            },
            Err(_) => None,
        };
        let save = loaded.unwrap_or_else(|| {
            let seed = std::env::var("GAME_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
            });
            Save::new(seed)
        });
        Game { chapter: chapter_1_1(), layout: Ch1Layout::generate(save.seed), save, save_path, save_problem }
    }

    pub fn def(&self, puzzle: usize) -> &PuzzleDef {
        &self.chapter.puzzles[puzzle]
    }

    pub fn instance(&self, puzzle: usize) -> Instance {
        Instance::new(self.def(puzzle).kind, &self.layout)
    }

    pub fn unlocked(&self, puzzle: usize) -> bool {
        self.save.is_unlocked(&self.chapter, &self.def(puzzle).id)
    }

    pub fn solved(&self, puzzle: usize) -> bool {
        self.save.is_solved(&self.def(puzzle).id)
    }

    pub fn charges_left(&self, puzzle: usize) -> u32 {
        self.save.charges_left(&self.chapter, &self.def(puzzle).id)
    }

    /// The first puzzle not yet solved, if any.
    pub fn next_unsolved(&self) -> Option<usize> {
        (0..self.chapter.puzzles.len()).find(|&i| !self.solved(i))
    }

    /// Write the save file. A failure is kept for display rather than stopping the game.
    pub fn persist(&mut self) {
        let result = self
            .save_path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&self.save_path, self.save.to_ron()));
        self.save_problem = result.err().map(|e| format!("Could not save to {}: {e}", self.save_path.display()));
    }
}

/// Seconds a station takes to recharge once its prototype charges are used up.
pub const RECHARGE_SECONDS: f32 = 20.0;

/// What the interface is showing and what the player has typed.
#[derive(Resource, Default)]
pub struct Ui {
    /// Station whose panel is open.
    pub station: Option<usize>,
    /// Station the avatar is standing at.
    pub near_station: Option<usize>,
    /// Text typed into the blueprint, per puzzle and input.
    pub drafts: [Vec<String>; 3],
    /// Why the last commit was refused (costs no charge).
    pub refusal: Option<String>,
    /// Result of the last finished test, per puzzle.
    pub results: [Option<Outcome>; 3],
    /// Time left until a station has recharged.
    pub recharging: [Option<f32>; 3],
    pub show_notebook: bool,
    pub show_handbook: bool,
    pub show_tutor: bool,
    pub calculator: String,
    pub chat: String,
}

impl Ui {
    /// The puzzle the player is working on: the open station, else the one they stand at.
    pub fn focus(&self, game: &Game) -> usize {
        self.station
            .or(self.near_station)
            .or(game.next_unsolved())
            .unwrap_or(game.chapter.puzzles.len() - 1)
    }
}
