//! Game state shared between systems: the chapter, the seeded layout, the save, and what the
//! interface is showing.

use bevy::prelude::*;
use physics_core::notebook::source;
use physics_core::content::{Chapter, Found, PuzzleDef, Tier, chapter_1_1, tier_1};
use physics_core::progress::{CommittedValue, Save};
use physics_core::puzzle::{Ch1Layout, Instance, Outcome, PuzzleKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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
    /// False when an unreadable save could not be moved aside: the game then writes nothing rather
    /// than overwrite it.
    pub may_write: bool,
    /// A test committed and judged, still playing out in the world. Its charge is already used, and
    /// the save file holds it as if it had finished, so quitting mid-test does not take it back; the
    /// game itself counts it only once it has played out (see [`Game::finish_test`]).
    pub pending: Option<PendingTest>,
}

/// A judged test that has not been shown yet.
#[derive(Clone, Debug)]
pub struct PendingTest {
    pub puzzle: usize,
    pub values: Vec<CommittedValue>,
    pub outcome: Outcome,
}

impl Game {
    /// Continue from the save file, or start a new game.
    ///
    /// `GAME_SAVE` overrides the save path and `GAME_SEED` the seed of a new game.
    pub fn load_or_new() -> Self {
        let save_path = PathBuf::from(std::env::var("GAME_SAVE").unwrap_or_else(|_| "saves/slot1.ron".into()));
        let opened = open_save(&save_path);
        let save = opened.save.unwrap_or_else(|| {
            let seed = std::env::var("GAME_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or_else(|| {
                std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64)
            });
            Save::new(seed)
        });
        let chapter = chapter_1_1();
        assert_eq!(chapter.puzzles.len(), PUZZLES, "the chapter's puzzles and the island's stations must match");
        Game {
            tier: tier_1(),
            chapter,
            layout: Ch1Layout::generate(save.seed),
            save,
            save_path,
            save_problem: opened.problem,
            may_write: opened.may_write,
            pending: None,
        }
    }

    /// A new game on `seed` that saves to a scratch file, for tests.
    #[cfg(test)]
    pub fn for_test(seed: u64) -> Self {
        let save_path = std::env::temp_dir().join(format!("fp-test-{seed}-{}-{:?}.ron", std::process::id(), std::thread::current().id()));
        Game {
            tier: tier_1(),
            chapter: chapter_1_1(),
            layout: Ch1Layout::generate(seed),
            save: Save::new(seed),
            save_path,
            save_problem: None,
            may_write: true,
            pending: None,
        }
    }

    pub fn def(&self, puzzle: usize) -> &PuzzleDef {
        &self.chapter.puzzles[puzzle]
    }

    /// The chapter's puzzle of this kind. Every kind has one station.
    pub fn puzzle_of(&self, kind: PuzzleKind) -> usize {
        self.chapter.puzzles.iter().position(|p| p.kind == kind).expect("the chapter has a puzzle of every kind")
    }

    /// Whether the chapter's puzzle of this kind is solved.
    pub fn kind_solved(&self, kind: PuzzleKind) -> bool {
        self.solved(self.puzzle_of(kind))
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

    /// What is placed at a puzzle's station: the cart's block on the cart, the light gate's notch,
    /// the number of blocks under the ledge launcher (see `Save::placed`).
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

    /// A test has been committed: use its charge now and keep the verdict until the world has shown
    /// it. The save file gets the whole test at once.
    pub fn begin_test(&mut self, puzzle: usize, values: Vec<CommittedValue>, outcome: Outcome) {
        let id = self.def(puzzle).id.clone();
        self.save.use_charge(&id);
        self.pending = Some(PendingTest { puzzle, values, outcome });
        self.persist();
    }

    /// The test has played out: now it counts. Returns its outcome.
    pub fn finish_test(&mut self) -> Option<Outcome> {
        let test = self.pending.take()?;
        let outcome = test.outcome.clone();
        apply_test(&self.chapter, &mut self.save, test);
        self.persist();
        Some(outcome)
    }

    /// Write the save file, with any test still playing out counted as finished. A failure is kept
    /// for display rather than stopping the game.
    pub fn persist(&mut self) {
        let text = match &self.pending {
            Some(test) => {
                let mut save = self.save.clone();
                apply_test(&self.chapter, &mut save, test.clone());
                save.to_ron()
            }
            None => self.save.to_ron(),
        };
        if !self.may_write {
            return;
        }
        let result = write_replacing(&self.save_path, &text);
        self.save_problem = result.err().map(|e| format!("Couldn't save to {}: {e}", self.save_path.display()));
    }
}

/// What came of opening the save file.
struct Opened {
    save: Option<Save>,
    problem: Option<String>,
    may_write: bool,
}

/// Read the save file. One that exists but can't be read is moved aside (to `slot1.ron.bak`, or
/// `slot1.ron.bak2` and on if that is taken) before a new game can overwrite it; if even that
/// fails, the new game is not saved at all.
fn open_save(path: &Path) -> Opened {
    let problem = match std::fs::read_to_string(path) {
        Ok(text) => match Save::from_ron(&text) {
            Ok(save) => return Opened { save: Some(save), problem: None, may_write: true },
            Err(e) => e.to_string(),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Opened { save: None, problem: None, may_write: true },
        Err(e) => e.to_string(),
    };
    let aside = (1..).map(|n| backup_path(path, n)).find(|p| !p.exists()).expect("some name is free");
    match std::fs::rename(path, &aside) {
        Ok(()) => Opened {
            save: None,
            problem: Some(format!("{} couldn't be read ({problem}), so a new game has started. The old file is now {}.", path.display(), aside.display())),
            may_write: true,
        },
        Err(e) => Opened {
            save: None,
            problem: Some(format!(
                "{} couldn't be read ({problem}) or moved aside ({e}). This new game won't be saved, so the old file stays as it is.",
                path.display()
            )),
            may_write: false,
        },
    }
}

/// `slot1.ron.bak` for `n` = 1, then `slot1.ron.bak2`, `slot1.ron.bak3`...
fn backup_path(path: &Path, n: u32) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(if n == 1 { ".bak".to_string() } else { format!(".bak{n}") });
    path.with_file_name(name)
}

/// Write `text` to a new file beside `path` and rename it over `path`, so a crash mid-write leaves
/// the old file whole.
fn write_replacing(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    let temporary = path.with_file_name(name);
    {
        use std::io::Write;
        let mut file = std::fs::File::create(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
    }
    std::fs::rename(&temporary, path)
}

/// Count a test that has been shown (its charge is already used): the attempt, and on a pass at the
/// drop tower the player's own g, which later puzzles use.
fn apply_test(chapter: &Chapter, save: &mut Save, test: PendingTest) {
    let def = &chapter.puzzles[test.puzzle];
    if test.outcome.pass && def.kind == PuzzleKind::Gravity {
        save.notebook.set("g", "gravitational acceleration (your calibration)", test.values[0].quantity, source::CALIBRATED);
    }
    save.record_attempt(&def.id, test.values, &test.outcome);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("fp-state-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_missing_save_starts_a_new_game_quietly() {
        let dir = scratch_dir("missing");
        let opened = open_save(&dir.join("slot1.ron"));
        assert!(opened.save.is_none() && opened.problem.is_none() && opened.may_write);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_unreadable_save_is_moved_aside_before_anything_overwrites_it() {
        let dir = scratch_dir("damaged");
        let path = dir.join("slot1.ron");
        std::fs::write(&path, "(seed: 3, half a file").unwrap();
        std::fs::write(dir.join("slot1.ron.bak"), "an older backup").unwrap();
        let opened = open_save(&path);
        assert!(opened.save.is_none() && opened.may_write);
        let problem = opened.problem.unwrap();
        assert!(problem.contains("slot1.ron.bak2"), "{problem}");
        assert!(!path.exists());
        assert_eq!(std::fs::read_to_string(dir.join("slot1.ron.bak2")).unwrap(), "(seed: 3, half a file");
        assert_eq!(std::fs::read_to_string(dir.join("slot1.ron.bak")).unwrap(), "an older backup");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saving_replaces_the_file_whole_and_leaves_nothing_behind() {
        let dir = scratch_dir("persist");
        let mut game = Game::for_test(11);
        game.save_path = dir.join("saves").join("slot1.ron");
        game.persist();
        assert!(game.save_problem.is_none(), "{:?}", game.save_problem);
        game.save.notebook.set("d", "a distance", physics_core::units::Quantity::metres(4.0), source::RULER);
        game.persist();
        let opened = open_save(&game.save_path);
        assert_eq!(opened.save.as_ref(), Some(&game.save));
        let files: Vec<_> = std::fs::read_dir(dir.join("saves")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(files, ["slot1.ron"]);

        game.may_write = false;
        game.save.notebook.set("e", "another distance", physics_core::units::Quantity::metres(5.0), source::RULER);
        game.persist();
        assert_ne!(open_save(&game.save_path).save.as_ref(), Some(&game.save), "nothing is written");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
