//! Game state shared between systems: the chapter, the seeded layout, the save, and what the
//! interface is showing.

use bevy::prelude::*;
use physics_core::content::{Chapter, Found, PuzzleDef, Tier, chapter_1_1, tier_1};
use physics_core::progress::Save;
use physics_core::puzzle::{Ch1Layout, Instance, Outcome};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Whether the title menu, with the intro playing behind it, or the game is running.
#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    #[default]
    Menu,
    Playing,
}

/// Number of puzzles (and stations) in the chapter.
pub const PUZZLES: usize = 5;

#[derive(Resource)]
pub struct Game {
    /// The tier the chapter belongs to: what the ship is for.
    pub tier: Tier,
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
        let chapter = chapter_1_1();
        assert_eq!(chapter.puzzles.len(), PUZZLES, "the chapter's puzzles and the island's stations must match");
        Game { tier: tier_1(), chapter, layout: Ch1Layout::generate(save.seed), save, save_path, save_problem }
    }

    /// A new game on `seed` that saves to a scratch file, for tests.
    #[cfg(test)]
    pub fn for_test(seed: u64) -> Self {
        let save_path = std::env::temp_dir().join(format!("fp-test-{seed}-{}-{:?}.ron", std::process::id(), std::thread::current().id()));
        Game { tier: tier_1(), chapter: chapter_1_1(), layout: Ch1Layout::generate(seed), save: Save::new(seed), save_path, save_problem: None }
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

    /// What is placed at a puzzle's station: the cargo crate on the cart, the light gate's notch,
    /// the number of crates under the ledge launcher (see `Save::placed`).
    pub fn placed(&self, puzzle: usize) -> u32 {
        self.save.placed.get(&self.def(puzzle).id).copied().unwrap_or(0)
    }

    /// Whether a handbook page can be read.
    pub fn page_found(&self, concept_id: &str) -> bool {
        self.save.is_found(&self.chapter, concept_id)
    }

    /// Write the handbook pages that `event` at a puzzle's station reveals, and queue the new ones as
    /// unread. Returns how many were new. The caller persists.
    pub fn find_pages(&mut self, ui: &mut Ui, puzzle: usize, event: PageEvent) -> usize {
        let id = self.def(puzzle).id.clone();
        let pages: Vec<String> = self
            .chapter
            .pages_found_at(&id)
            .into_iter()
            .filter(|c| matches!((&c.found, event), (Found::Demo(_), PageEvent::Demo) | (Found::FirstCommit(_), PageEvent::Commit)))
            .map(|c| c.id.clone())
            .collect();
        let mut new = 0;
        for page in pages {
            if self.save.find(&page) {
                ui.unread_pages.push(page);
                new += 1;
            }
        }
        new
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

/// What happened at a station that may write handbook pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PageEvent {
    /// Its demonstration was watched.
    Demo,
    /// A value was committed there (tested or refused).
    Commit,
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
    pub drafts: [Vec<String>; PUZZLES],
    /// Why the last commit was refused (costs no charge).
    pub refusal: Option<String>,
    /// Result of the last finished test, per puzzle.
    pub results: [Option<Outcome>; PUZZLES],
    /// Time left until a station has recharged.
    pub recharging: [Option<f32>; PUZZLES],
    /// Whether the notebook, the handbook and Lumen's panel are expanded. They are always on
    /// screen; reduced, they show only their header (Lumen: its eye).
    pub notebook_open: bool,
    pub handbook_open: bool,
    pub tutor_open: bool,
    /// Bottom edge of the dock at the top left, in interface points, so Lumen's panel can stay
    /// clear of it.
    pub dock_bottom: f32,
    pub calculator: String,
    pub chat: String,
    /// The choice picked in a station's demonstration before its first run.
    pub guess: [Option<usize>; PUZZLES],
    /// Handbook pages written but not read yet, by concept id, oldest first.
    pub unread_pages: Vec<String>,
    /// The page the handbook should open at.
    pub handbook_focus: Option<String>,
    /// Each handbook page's picture: when it started playing, and whether it is paused.
    pub anims: BTreeMap<String, Anim>,
    /// The choice picked for each handbook quiz question, by page id and question. Not saved: the
    /// save keeps only which questions have been answered right.
    pub quiz_picks: BTreeMap<(String, usize), usize>,
    /// The page whose quiz should open and come into view.
    pub quiz_focus: Option<String>,
}

/// A handbook picture's clock, in seconds of the interface's time.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anim {
    pub start: f64,
    /// When it was paused, if it is.
    pub paused_at: Option<f64>,
}

impl Ui {
    /// Open the handbook at a page.
    pub fn open_page(&mut self, concept_id: &str) {
        self.handbook_open = true;
        self.handbook_focus = Some(concept_id.to_string());
    }

    /// The puzzle the player is working on: the open station, else the one they stand at.
    pub fn focus(&self, game: &Game) -> usize {
        self.station
            .or(self.near_station)
            .or(game.next_unsolved())
            .unwrap_or(game.chapter.puzzles.len() - 1)
    }
}
