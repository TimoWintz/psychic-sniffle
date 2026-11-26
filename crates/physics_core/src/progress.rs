//! The save file: seed, notebook, and what has been attempted and solved.

use crate::content::Chapter;
use crate::notebook::Notebook;
use crate::puzzle::Outcome;
use crate::units::Quantity;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommittedValue {
    pub label: String,
    pub quantity: Quantity,
    pub unit: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attempt {
    pub values: Vec<CommittedValue>,
    pub pass: bool,
    pub headline: String,
    pub detail: String,
}

/// How good the fabricated part is. Decided by how many tests it took.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    /// Solved on the first test.
    Pristine,
    Serviceable,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PuzzleProgress {
    pub attempts: Vec<Attempt>,
    /// Tests used since the station last recharged.
    pub charges_used: u32,
    pub solved: bool,
}

impl PuzzleProgress {
    pub fn quality(&self) -> Option<Quality> {
        if !self.solved {
            return None;
        }
        // Attempts made after solving do not change the part that was built.
        let first_pass = self.attempts.iter().position(|a| a.pass)?;
        Some(if first_pass == 0 { Quality::Pristine } else { Quality::Serviceable })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Save {
    pub version: u32,
    pub seed: u64,
    pub notebook: Notebook,
    pub puzzles: BTreeMap<String, PuzzleProgress>,
    /// Handbook pages written so far, by concept id. (Saves from before pages were found load
    /// without it; see [`Save::is_found`].)
    #[serde(default)]
    pub found: BTreeSet<String>,
    /// Demonstrations watched, by puzzle id, with the choice made before the first run (if any).
    #[serde(default)]
    pub demos: BTreeMap<String, Option<usize>>,
    /// What the player has placed at each station, by puzzle id: the block on the rail cart
    /// (0 or 1), the light gate's notch on the drop tower (its drop below the release point in metres,
    /// 0 for none), the number of blocks under the ledge launcher. Kept between sessions so the world
    /// looks as it was left.
    #[serde(default)]
    pub placed: BTreeMap<String, u32>,
    /// Handbook quizzes: for each page id, the questions answered right at least once. Optional
    /// practice; it changes nothing in the world.
    #[serde(default)]
    pub quizzes: BTreeMap<String, BTreeSet<usize>>,
    /// The side-machines off the road, by id: what the player has built on each and how far it has
    /// got. Optional; nothing on the main route reads it.
    #[serde(default)]
    pub sides: BTreeMap<String, SideProgress>,
}

/// A side-machine's state (see "Side-machines" in the game design).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SideProgress {
    /// What is built on it, as its kind keeps it (the beam: blocks on each seat, see
    /// `beam::Placement::to_saved`).
    #[serde(default)]
    pub placed: Vec<u32>,
    /// How many of its cases have held.
    #[serde(default)]
    pub held: u32,
    /// Whether it has woken: every case done.
    #[serde(default)]
    pub woken: bool,
}

impl Save {
    /// 2: the haul track became puzzle 1.1.1 and the later puzzles moved up one, so a version 1 save's
    /// ids name different puzzles.
    pub const VERSION: u32 = 2;

    pub fn new(seed: u64) -> Self {
        Save {
            version: Self::VERSION,
            seed,
            notebook: Notebook::default(),
            puzzles: BTreeMap::new(),
            found: BTreeSet::new(),
            demos: BTreeMap::new(),
            placed: BTreeMap::new(),
            quizzes: BTreeMap::new(),
            sides: BTreeMap::new(),
        }
    }

    pub fn to_ron(&self) -> String {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default()).expect("a save serialises")
    }

    pub fn from_ron(source: &str) -> Result<Save, String> {
        let save: Save = ron::from_str(source).map_err(|e| e.to_string())?;
        if save.version != Self::VERSION {
            return Err(format!("save version {} is not supported (expected {})", save.version, Self::VERSION));
        }
        Ok(save)
    }

    /// Whether nothing has happened in this game yet: nothing measured, noted, watched, placed,
    /// read or tested.
    pub fn untouched(&self) -> bool {
        *self == Save::new(self.seed)
    }

    pub fn progress(&self, puzzle_id: &str) -> PuzzleProgress {
        self.puzzles.get(puzzle_id).cloned().unwrap_or_default()
    }

    pub fn is_solved(&self, puzzle_id: &str) -> bool {
        self.puzzles.get(puzzle_id).is_some_and(|p| p.solved)
    }

    /// A puzzle is open when everything it requires has been solved.
    pub fn is_unlocked(&self, chapter: &Chapter, puzzle_id: &str) -> bool {
        chapter.puzzle(puzzle_id).is_some_and(|p| p.requires.iter().all(|r| self.is_solved(r)))
    }

    pub fn chapter_complete(&self, chapter: &Chapter) -> bool {
        chapter.puzzles.iter().all(|p| self.is_solved(&p.id))
    }

    pub fn charges_left(&self, chapter: &Chapter, puzzle_id: &str) -> u32 {
        let total = chapter.puzzle(puzzle_id).map_or(0, |p| p.charges);
        total.saturating_sub(self.progress(puzzle_id).charges_used)
    }

    /// Write a handbook page. Returns `true` if it was not written before.
    pub fn find(&mut self, concept_id: &str) -> bool {
        self.found.insert(concept_id.to_string())
    }

    /// Whether a handbook page can be read. Solving the puzzle whose station writes it (or waking
    /// the side-machine that does) counts as having found it, so a page is never missing for a job
    /// that is done.
    pub fn is_found(&self, chapter: &Chapter, concept_id: &str) -> bool {
        self.found.contains(concept_id) || chapter.concept(concept_id).is_some_and(|c| self.is_solved(c.found.puzzle()) || self.side_woken(c.found.puzzle()))
    }

    /// A side-machine's state, empty if nothing has been done there.
    pub fn side(&self, side_id: &str) -> SideProgress {
        self.sides.get(side_id).cloned().unwrap_or_default()
    }

    pub fn side_woken(&self, side_id: &str) -> bool {
        self.sides.get(side_id).is_some_and(|s| s.woken)
    }

    pub fn demo_watched(&self, puzzle_id: &str) -> bool {
        self.demos.contains_key(puzzle_id)
    }

    /// The choice made before a demonstration's first run, if one was made.
    pub fn first_guess(&self, puzzle_id: &str) -> Option<usize> {
        self.demos.get(puzzle_id).copied().flatten()
    }

    /// Note that a demonstration ran. Only the guess before the first run is kept.
    pub fn record_demo(&mut self, puzzle_id: &str, guess: Option<usize>) {
        self.demos.entry(puzzle_id.to_string()).or_insert(guess);
    }

    /// Note a right answer to a page's quiz question. Returns `true` if it finished the page's quiz
    /// just now.
    pub fn answer_quiz(&mut self, chapter: &Chapter, concept_id: &str, question: usize) -> bool {
        let was_done = self.quiz_done(chapter, concept_id);
        self.quizzes.entry(concept_id.to_string()).or_default().insert(question);
        !was_done && self.quiz_done(chapter, concept_id)
    }

    /// Whether every question of a page's quiz has been answered right. A page without a quiz is
    /// never done.
    pub fn quiz_done(&self, chapter: &Chapter, concept_id: &str) -> bool {
        let Some(concept) = chapter.concept(concept_id).filter(|c| !c.quiz.is_empty()) else { return false };
        let right = self.quizzes.get(concept_id);
        (0..concept.quiz.len()).all(|q| right.is_some_and(|r| r.contains(&q)))
    }

    pub fn recharge(&mut self, puzzle_id: &str) {
        if let Some(p) = self.puzzles.get_mut(puzzle_id) {
            p.charges_used = 0;
        }
    }

    /// Record a judged test: uses one charge and marks the puzzle solved on a pass.
    pub fn record(&mut self, puzzle_id: &str, values: Vec<CommittedValue>, outcome: &Outcome) {
        self.use_charge(puzzle_id);
        self.record_attempt(puzzle_id, values, outcome);
    }

    /// Use one of a station's charges, as committing a test does.
    pub fn use_charge(&mut self, puzzle_id: &str) {
        self.puzzles.entry(puzzle_id.to_string()).or_default().charges_used += 1;
    }

    /// Record what a test showed, without using a charge ([`Save::use_charge`] did that at commit),
    /// and mark the puzzle solved on a pass.
    pub fn record_attempt(&mut self, puzzle_id: &str, values: Vec<CommittedValue>, outcome: &Outcome) {
        let progress = self.puzzles.entry(puzzle_id.to_string()).or_default();
        progress.solved |= outcome.pass;
        progress.attempts.push(Attempt { values, pass: outcome.pass, headline: outcome.headline.clone(), detail: outcome.detail.clone() });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::chapter_1_1;
    use crate::puzzle::{Ch1Layout, Instance, PuzzleKind};

    fn attempt(save: &mut Save, id: &str, kind: PuzzleKind, scale: f64) -> bool {
        let puzzle = Instance::new(kind, &Ch1Layout::generate(save.seed));
        let mut inputs = puzzle.solution().inputs();
        let key = puzzle.inputs().last().unwrap().key;
        let value = &mut inputs.get_mut(key).unwrap().value;
        if kind == PuzzleKind::Channel && scale != 1.0 {
            // The dial turns only so far: a miss is the top or the bottom of its band.
            let layout = &puzzle.layout;
            *value = if scale < 1.0 { layout.channel_speed_min } else { layout.channel_speed_max };
        } else {
            *value *= scale;
        }
        let outcome = puzzle.evaluate(&inputs).unwrap();
        save.record(id, Vec::new(), &outcome);
        outcome.pass
    }

    #[test]
    fn puzzles_unlock_in_order() {
        let chapter = chapter_1_1();
        let mut save = Save::new(42);
        assert!(save.is_unlocked(&chapter, "1.1.0"));
        assert!(!save.is_unlocked(&chapter, "1.1.1"));
        assert!(attempt(&mut save, "1.1.0", PuzzleKind::Rail, 1.0));
        assert!(save.is_unlocked(&chapter, "1.1.1"));
        assert!(!save.is_unlocked(&chapter, "1.1.2"));
        assert!(attempt(&mut save, "1.1.1", PuzzleKind::Chase, 1.0));
        assert!(save.is_unlocked(&chapter, "1.1.2"));
        assert!(!save.is_unlocked(&chapter, "1.1.3"));
        assert!(attempt(&mut save, "1.1.2", PuzzleKind::Gravity, 1.0));
        assert!(save.is_unlocked(&chapter, "1.1.3"));
        assert!(!save.is_unlocked(&chapter, "1.1.4"));
        assert!(!save.chapter_complete(&chapter));
        assert!(attempt(&mut save, "1.1.3", PuzzleKind::Channel, 1.0));
        assert!(attempt(&mut save, "1.1.4", PuzzleKind::Ridge, 1.0));
        assert!(save.chapter_complete(&chapter));
    }

    #[test]
    fn charges_and_quality() {
        let chapter = chapter_1_1();
        let mut save = Save::new(42);
        assert_eq!(save.charges_left(&chapter, "1.1.3"), 3);
        assert!(!attempt(&mut save, "1.1.3", PuzzleKind::Channel, 0.5));
        assert!(!attempt(&mut save, "1.1.3", PuzzleKind::Channel, 1.5));
        assert_eq!(save.charges_left(&chapter, "1.1.3"), 1);
        assert_eq!(save.progress("1.1.3").quality(), None);
        assert!(attempt(&mut save, "1.1.3", PuzzleKind::Channel, 1.0));
        assert_eq!(save.charges_left(&chapter, "1.1.3"), 0);
        assert_eq!(save.progress("1.1.3").quality(), Some(Quality::Serviceable));
        save.recharge("1.1.3");
        assert_eq!(save.charges_left(&chapter, "1.1.3"), 3);

        assert!(attempt(&mut save, "1.1.2", PuzzleKind::Gravity, 1.0));
        assert_eq!(save.progress("1.1.2").quality(), Some(Quality::Pristine));
    }

    #[test]
    fn a_save_is_untouched_until_anything_happens() {
        let mut save = Save::new(5);
        assert!(save.untouched());
        save.notebook.set("d", "a distance", Quantity::metres(3.0), "ruler");
        assert!(!save.untouched());
        let mut watched = Save::new(5);
        watched.record_demo("1.1.0", None);
        assert!(!watched.untouched());
        let mut placed = Save::new(5);
        placed.placed.insert("1.1.0".into(), 1);
        assert!(!placed.untouched());
        let mut beam = Save::new(5);
        beam.sides.entry("1.1.s1".into()).or_default().placed = vec![0, 1];
        assert!(!beam.untouched());
    }

    #[test]
    fn a_side_machine_is_kept_and_its_page_comes_with_its_wake() {
        let chapter = chapter_1_1();
        let mut save = Save::new(6);
        assert_eq!(save.side("1.1.s1"), SideProgress::default());
        assert!(!save.is_found(&chapter, "a_rule_predicts"));
        let beam = save.sides.entry("1.1.s1".into()).or_default();
        beam.placed = vec![0, 0, 0, 0, 0, 0, 2, 0, 0, 1];
        beam.held = 3;
        beam.woken = true;
        assert!(save.side_woken("1.1.s1"));
        assert!(save.is_found(&chapter, "a_rule_predicts"));
        // Nothing on the main route moves with it.
        assert!(!save.is_unlocked(&chapter, "1.1.1") && !save.chapter_complete(&chapter));
        assert_eq!(Save::from_ron(&save.to_ron()).unwrap(), save);
    }

    #[test]
    fn save_round_trips() {
        let mut save = Save::new(9);
        save.notebook.set("g", "gravity", Quantity::acceleration(8.25), "calculated");
        save.placed.insert("1.1.3".into(), 4);
        save.placed.insert("1.1.2".into(), 9);
        save.placed.insert("1.1.0".into(), 1);
        save.answer_quiz(&chapter_1_1(), "velocity", 1);
        attempt(&mut save, "1.1.2", PuzzleKind::Gravity, 1.0);
        let text = save.to_ron();
        assert_eq!(Save::from_ron(&text).unwrap(), save);
        assert!(Save::from_ron(&text.replace("version: 2", "version: 99")).is_err());
    }

    #[test]
    fn saves_from_before_pages_and_demonstrations_still_load() {
        let old = r#"(version: 2, seed: 5, notebook: (entries: []), puzzles: {"1.1.0": (attempts: [], charges_used: 1, solved: true)})"#;
        let save = Save::from_ron(old).unwrap();
        assert!(save.found.is_empty() && save.demos.is_empty() && save.placed.is_empty() && save.quizzes.is_empty() && save.sides.is_empty());
        // The tutorial is done, so its page is there; the drop tower's is not yet.
        let chapter = chapter_1_1();
        assert!(save.is_found(&chapter, "units"));
        assert!(!save.is_found(&chapter, "free_fall"));
    }

    #[test]
    fn saves_from_before_the_haul_track_are_refused() {
        // Their "1.1.1" was the drop tower; read as the haul track it would be wrong.
        let old = r#"(version: 1, seed: 5, notebook: (entries: []), puzzles: {"1.1.1": (attempts: [], charges_used: 1, solved: true)})"#;
        let err = Save::from_ron(old).unwrap_err();
        assert!(err.contains("save version 1 is not supported"), "{err}");
    }

    #[test]
    fn pages_and_demonstrations_are_kept() {
        let chapter = chapter_1_1();
        let mut save = Save::new(3);
        assert!(save.find("free_fall"));
        assert!(!save.find("free_fall"), "a page is written once");
        assert!(save.is_found(&chapter, "free_fall"));

        assert!(!save.demo_watched("1.1.3"));
        save.record_demo("1.1.3", Some(0));
        save.record_demo("1.1.3", Some(2));
        assert!(save.demo_watched("1.1.3"));
        assert_eq!(save.first_guess("1.1.3"), Some(0), "only the guess before the first run counts");
        save.record_demo("1.1.4", None);
        assert!(save.demo_watched("1.1.4"));
        assert_eq!(save.first_guess("1.1.4"), None);

        assert_eq!(Save::from_ron(&save.to_ron()).unwrap(), save);
    }

    #[test]
    fn a_quiz_is_done_once_every_question_is_answered_right() {
        let chapter = chapter_1_1();
        let questions = chapter.concept("free_fall").unwrap().quiz.len();
        let mut save = Save::new(4);
        assert!(!save.quiz_done(&chapter, "free_fall"));
        for q in 0..questions - 1 {
            assert!(!save.answer_quiz(&chapter, "free_fall", q));
        }
        // Answering the same question again finishes nothing.
        assert!(!save.answer_quiz(&chapter, "free_fall", 0));
        assert!(!save.quiz_done(&chapter, "free_fall"));
        assert!(save.answer_quiz(&chapter, "free_fall", questions - 1), "the last question finishes the quiz");
        assert!(save.quiz_done(&chapter, "free_fall"));
        assert!(!save.answer_quiz(&chapter, "free_fall", 0), "a quiz is finished once");
        assert!(!save.quiz_done(&chapter, "velocity"));
        assert!(!save.quiz_done(&chapter, "no_such_page"));
        assert_eq!(Save::from_ron(&save.to_ron()).unwrap(), save);
    }
}
