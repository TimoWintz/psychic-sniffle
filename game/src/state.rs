//! Game state shared between systems: the chapter, the seeded layout, the save, and what the
//! interface is showing.

use bevy::ecs::schedule::ScheduleLabel;
use bevy::prelude::*;
use physics_core::content::{Chapter, Found, PuzzleDef, Tier, chapter_1_1, tier_1};
use physics_core::notebook::source;
use physics_core::progress::{CommittedValue, Save};
use physics_core::puzzle::{Ch1Layout, Instance, Outcome, PuzzleKind};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

/// Whether the title menu, with the intro playing behind it, or the game is running.
#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Mode {
    #[default]
    Menu,
    Playing,
}

/// Marker on every entity that belongs to the loaded island/save and is despawned on reload.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct WorldEntity;

/// Schedule that spawns save-dependent world entities from `Game`, `VoxelWorld`, and `Sites`.
/// Runs once at startup and again whenever a different save or new game is loaded.
#[derive(ScheduleLabel, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BuildWorld;

/// Run condition: true while the game is not paused.
pub fn not_paused(ui: Res<Ui>) -> bool {
    !ui.paused
}

/// Number of puzzles (and stations) in the chapter.
pub const PUZZLES: usize = 5;

/// Number of save slots shown in the main menu.
pub const SLOTS: usize = 3;

/// Default directory holding numbered save slots.
pub fn saves_dir() -> PathBuf {
    PathBuf::from("saves")
}

/// Path of a numbered save slot (`slot` in `0..SLOTS` -> `slot1.ron` .. `slot3.ron`).
pub fn slot_path(dir: &Path, slot: usize) -> PathBuf {
    dir.join(format!("slot{}.ron", slot + 1))
}

/// Read-only inspection of one save file for the main menu.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotStatus {
    Empty,
    Occupied { solved: usize, total: usize, complete: bool, readings: usize, pages: usize, beam_woken: bool, seed: u64, modified: Option<SystemTime> },
    Damaged { problem: String, modified: Option<SystemTime> },
}

/// Summary of one numbered save slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotSummary {
    pub index: usize,
    pub path: PathBuf,
    pub status: SlotStatus,
}

/// Inspect a save file without modifying or backing it up.
pub fn inspect_slot(chapter: &Chapter, path: &Path) -> SlotStatus {
    let modified = std::fs::metadata(path).ok().and_then(|m| m.modified().ok());
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return SlotStatus::Empty,
        Err(e) => return SlotStatus::Damaged { problem: e.to_string(), modified },
    };
    match Save::from_ron(&text) {
        Ok(save) => {
            let solved = chapter.puzzles.iter().filter(|p| save.is_solved(&p.id)).count();
            let total = chapter.puzzles.len();
            let pages = chapter.concepts.iter().filter(|c| save.is_found(chapter, &c.id)).count();
            let beam_woken = save.side_woken(crate::beam::BEAM_ID);
            SlotStatus::Occupied {
                solved,
                total,
                complete: save.chapter_complete(chapter),
                readings: save.notebook.entries.len(),
                pages,
                beam_woken,
                seed: save.seed,
                modified,
            }
        }
        Err(problem) => SlotStatus::Damaged { problem, modified },
    }
}

/// Inspect all standard save slots in `dir`.
pub fn inspect_slots(chapter: &Chapter, dir: &Path) -> [SlotSummary; SLOTS] {
    std::array::from_fn(|index| {
        let path = slot_path(dir, index);
        let status = inspect_slot(chapter, &path);
        SlotSummary { index, path, status }
    })
}

/// Index of the most recently modified occupied slot, if any.
pub fn latest_slot(slots: &[SlotSummary]) -> Option<usize> {
    let mut best: Option<(usize, Option<SystemTime>)> = None;
    for slot in slots {
        let SlotStatus::Occupied { modified, .. } = slot.status else { continue };
        let replace = match best {
            None => true,
            Some((_, prev)) => match (prev, modified) {
                (Some(a), Some(b)) => b > a,
                (None, Some(_)) => true,
                _ => false,
            },
        };
        if replace {
            best = Some((slot.index, modified));
        }
    }
    best.map(|(i, _)| i)
}

/// Delete a save file if it exists. Missing files succeed quietly.
pub fn delete_slot(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// A fresh seed for a new game, honoring `GAME_SEED` when set.
pub fn fresh_seed() -> u64 {
    if let Ok(raw) = std::env::var("GAME_SEED")
        && let Ok(seed) = raw.parse()
    {
        return seed;
    }
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
    let salt = COUNTER.fetch_add(1, Ordering::Relaxed).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    let seed = nanos ^ salt;
    if seed == 0 { 1 } else { seed }
}

/// Deferred world/session action processed by [`crate::session`].
#[derive(Clone, Debug)]
pub enum SessionAction {
    /// Load an existing save (or open `path`), rebuilding the world if needed, then start playing.
    LoadAndPlay { path: PathBuf },
    /// Create a brand-new save at `path`, persist it, rebuild the world, then start playing.
    NewGameAndPlay { path: PathBuf },
    /// Leave gameplay and return to the title menu.
    ReturnToMenu,
}

/// Tracks pending session transitions and whether the currently built world has been played in.
#[derive(Resource, Default)]
pub struct SessionControl {
    pub pending: Option<SessionAction>,
    /// True once gameplay has been entered on the current world, so re-entering from the main menu
    /// rebuilds cleanly from the save file.
    pub world_dirty: bool,
}

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
    /// Continue from the most recent save file, or prepare a new game.
    ///
    /// `GAME_SAVE` overrides the save path and `GAME_SEED` the seed of a new game.
    pub fn load_or_new() -> Self {
        if let Ok(path) = std::env::var("GAME_SAVE") {
            return Self::from_path(PathBuf::from(path));
        }
        let chapter = chapter_1_1();
        let dir = saves_dir();
        let slots = inspect_slots(&chapter, &dir);
        let slot = latest_slot(&slots).unwrap_or(0);
        Self::from_path(slot_path(&dir, slot))
    }

    /// Open a save file at `path`, or start a new game on a fresh seed if it does not exist.
    /// An unreadable file is safely moved aside before anything can overwrite it.
    pub fn from_path(save_path: PathBuf) -> Self {
        let opened = open_save(&save_path);
        let save = opened.save.unwrap_or_else(|| Save::new(fresh_seed()));
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

    /// Start a brand-new game at `save_path` on `seed` and write it immediately so the slot is
    /// recorded on disk. If an unreadable file already occupies `save_path`, it is moved aside to a
    /// `.bak` file first rather than overwritten.
    pub fn new_at(save_path: PathBuf, seed: u64) -> Self {
        let mut save_problem = None;
        let mut may_write = true;
        if save_path.exists() {
            let valid = std::fs::read_to_string(&save_path).ok().and_then(|t| Save::from_ron(&t).ok()).is_some();
            if !valid {
                let aside = (1..).map(|n| backup_path(&save_path, n)).find(|p| !p.exists()).expect("some name is free");
                match std::fs::rename(&save_path, &aside) {
                    Ok(()) => {
                        save_problem = Some(format!("The unreadable save at {} was moved to {}.", save_path.display(), aside.display()));
                    }
                    Err(e) => {
                        save_problem = Some(format!(
                            "{} couldn't be moved aside ({e}). This new game won't be saved, so the old file stays as it is.",
                            save_path.display()
                        ));
                        may_write = false;
                    }
                }
            }
        }
        let chapter = chapter_1_1();
        assert_eq!(chapter.puzzles.len(), PUZZLES, "the chapter's puzzles and the island's stations must match");
        let save = Save::new(seed);
        let mut game = Game { tier: tier_1(), chapter, layout: Ch1Layout::generate(save.seed), save, save_path, save_problem, may_write, pending: None };
        if game.may_write {
            let previous_problem = game.save_problem.take();
            game.persist();
            if game.save_problem.is_none() {
                game.save_problem = previous_problem;
            }
        }
        game
    }

    /// Human-readable label for the current save slot or file.
    pub fn slot_label(&self) -> String {
        let dir = saves_dir();
        for i in 0..SLOTS {
            if self.save_path == slot_path(&dir, i) {
                return format!("Slot {}", i + 1);
            }
        }
        self.save_path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| self.save_path.display().to_string())
    }

    /// Whether `GAME_SAVE` is overriding the standard save slots.
    pub fn custom_save(&self) -> bool {
        let dir = saves_dir();
        !(0..SLOTS).any(|i| self.save_path == slot_path(&dir, i))
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

/// Which text field most recently had focus, so clicking a notebook variable inserts it there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputTarget {
    Calculator,
    Blueprint(usize, usize),
}

/// What the interface is showing and what the player has typed.
#[derive(Resource, Default)]
pub struct Ui {
    /// Station whose panel is open.
    pub station: Option<usize>,
    /// Station the avatar is standing at.
    pub near_station: Option<usize>,
    /// Whether the beam's console is open, and whether the avatar stands at it (see `beam`).
    pub beam_open: bool,
    pub near_beam: bool,
    /// Set by a console's "Briefing" button to reopen Lumen's station dialogue chips.
    pub request_briefing: bool,
    /// Set by a station's "Try it first" button to open Lumen's conceptual experiment prompt.
    pub request_experiment: bool,
    /// Set by a station's "Replay experiment" button, beside the run it queues, so Lumen opens the
    /// reveal again when that run ends.
    pub request_replay: bool,
    /// Text typed into the blueprint, per puzzle and input.
    pub drafts: [Vec<String>; PUZZLES],
    /// Why the last commit was refused (costs no charge).
    pub refusal: Option<String>,
    /// Result of the last finished test, per puzzle.
    pub results: [Option<Outcome>; PUZZLES],
    /// Scroll the station panel down to the result section on the next frame (set when a test ends).
    pub scroll_to_result: [bool; PUZZLES],
    /// Time left until a station has recharged.
    pub recharging: [Option<f32>; PUZZLES],
    /// Whether the notebook, the handbook and Lumen's panel are expanded. They are always on
    /// screen; reduced, they show only their header (Lumen: its eye).
    pub notebook_open: bool,
    pub handbook_open: bool,
    pub tutor_open: bool,
    /// Whether the compact controls reference card (F1 or `?`) is open.
    pub controls_open: bool,
    /// Whether the pause menu is open, freezing gameplay and physics.
    pub paused: bool,
    /// Tracks whether the handbook was open on the previous frame so closing it clears unread badges.
    pub handbook_was_open: bool,
    /// Screen rectangles (`[min_x, min_y, max_x, max_y]`, interface points) covered by HUD panels
    /// this frame, so Lumen's floating target labels stay clear of them.
    pub occupied: Vec<[f32; 4]>,
    /// Bottom edge of the dock at the top left, in interface points, so Lumen's panel can stay
    /// clear of it.
    pub dock_bottom: f32,
    pub calculator: String,
    /// Recent calculator expressions (oldest first), for Up/Down recall.
    pub calc_history: Vec<String>,
    /// Position in `calc_history` while stepping through it with Up/Down.
    pub calc_history_idx: Option<usize>,
    /// Unsubmitted calculator text kept while browsing `calc_history`.
    pub calc_draft_saved: String,
    /// Most recently focused calculator or blueprint field, for click-to-insert from the notebook.
    pub last_input: Option<InputTarget>,
    /// Notebook entry currently being renamed inline: `(original_name, draft_name)`.
    pub renaming_entry: Option<(String, String)>,
    /// Whether the inline rename text box should take keyboard focus on the next frame.
    pub focus_rename: bool,
    /// Validation message shown when the player tries to save an empty or invalid notebook rename.
    pub rename_submit_error: Option<String>,
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
    /// Whether a console is open, a station's or the beam's: the avatar stays at it.
    pub fn at_console(&self) -> bool {
        self.station.is_some() || self.beam_open
    }

    /// Open the handbook at a page.
    pub fn open_page(&mut self, concept_id: &str) {
        self.handbook_open = true;
        self.handbook_focus = Some(concept_id.to_string());
    }

    /// The puzzle the player is working on: the open station, else the one they stand at.
    pub fn focus(&self, game: &Game) -> usize {
        self.station.or(self.near_station).or(game.next_unsolved()).unwrap_or(game.chapter.puzzles.len() - 1)
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

    #[test]
    fn slot_inspection_reports_empty_occupied_and_damaged_without_modifying_files() {
        let dir = scratch_dir("slots");
        let chapter = chapter_1_1();
        let slots = inspect_slots(&chapter, &dir);
        assert_eq!(slots.len(), SLOTS);
        assert!(slots.iter().all(|s| s.status == SlotStatus::Empty));
        assert_eq!(latest_slot(&slots), None);

        // Write a valid save to slot 2 and a damaged file to slot 3.
        let mut save = Save::new(77);
        save.puzzles.entry(chapter.puzzles[0].id.clone()).or_default().solved = true;
        save.notebook.set("d1", "distance", physics_core::units::Quantity::metres(8.0), source::RULER);
        write_replacing(&slot_path(&dir, 1), &save.to_ron()).unwrap();
        std::fs::write(slot_path(&dir, 2), "not valid ron").unwrap();

        let slots = inspect_slots(&chapter, &dir);
        assert_eq!(slots[0].status, SlotStatus::Empty);
        match &slots[1].status {
            SlotStatus::Occupied { solved, total, complete, readings, seed, .. } => {
                assert_eq!(*solved, 1);
                assert_eq!(*total, PUZZLES);
                assert!(!complete);
                assert_eq!(*readings, 1);
                assert_eq!(*seed, 77);
            }
            other => panic!("expected occupied slot, got {other:?}"),
        }
        assert!(matches!(slots[2].status, SlotStatus::Damaged { .. }));
        // Read-only inspection leaves the damaged file untouched.
        assert!(slot_path(&dir, 2).exists());
        assert_eq!(latest_slot(&slots), Some(1));

        // Deleting a slot removes it and makes it empty again.
        delete_slot(&slot_path(&dir, 1)).unwrap();
        delete_slot(&slot_path(&dir, 1)).unwrap(); // idempotent
        assert_eq!(inspect_slot(&chapter, &slot_path(&dir, 1)), SlotStatus::Empty);

        // Starting a new game over a damaged slot backs up the damaged file first.
        let game = Game::new_at(slot_path(&dir, 2), 99);
        assert_eq!(game.save.seed, 99);
        assert!(dir.join("slot3.ron.bak").exists());
        assert!(matches!(inspect_slot(&chapter, &slot_path(&dir, 2)), SlotStatus::Occupied { seed: 99, .. }));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
