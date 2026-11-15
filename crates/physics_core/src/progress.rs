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
    /// What the player has built at each station, by puzzle id: the number of crates under the ledge
    /// launcher. Kept between sessions so the world looks as it was left.
    #[serde(default)]
    pub placed: BTreeMap<String, u32>,
}

impl Save {
    pub const VERSION: u32 = 1;

    pub fn new(seed: u64) -> Self {
        Save {
            version: Self::VERSION,
            seed,
            notebook: Notebook::default(),
            puzzles: BTreeMap::new(),
            found: BTreeSet::new(),
            demos: BTreeMap::new(),
            placed: BTreeMap::new(),
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

    /// Whether a handbook page can be read. Solving the puzzle whose station writes it counts as
    /// having found it, so a page is never missing for a puzzle that is done.
    pub fn is_found(&self, chapter: &Chapter, concept_id: &str) -> bool {
        self.found.contains(concept_id) || chapter.concept(concept_id).is_some_and(|c| self.is_solved(c.found.puzzle()))
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

    pub fn recharge(&mut self, puzzle_id: &str) {
        if let Some(p) = self.puzzles.get_mut(puzzle_id) {
            p.charges_used = 0;
        }
    }

    /// Record a judged test: uses one charge and marks the puzzle solved on a pass.
    pub fn record(&mut self, puzzle_id: &str, values: Vec<CommittedValue>, outcome: &Outcome) {
        let progress = self.puzzles.entry(puzzle_id.to_string()).or_default();
        progress.charges_used += 1;
        progress.solved |= outcome.pass;
        progress.attempts.push(Attempt {
            values,
            pass: outcome.pass,
            headline: outcome.headline.clone(),
            detail: outcome.detail.clone(),
        });
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
        *value *= scale;
        if key == "stack" {
            // Crates come whole.
            *value = value.round();
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
        assert!(attempt(&mut save, "1.1.1", PuzzleKind::Gravity, 1.0));
        assert!(save.is_unlocked(&chapter, "1.1.2"));
        assert!(!save.is_unlocked(&chapter, "1.1.3"));
        assert!(!save.chapter_complete(&chapter));
        assert!(attempt(&mut save, "1.1.2", PuzzleKind::Canyon, 1.0));
        assert!(attempt(&mut save, "1.1.3", PuzzleKind::Ridge, 1.0));
        assert!(save.chapter_complete(&chapter));
    }

    #[test]
    fn charges_and_quality() {
        let chapter = chapter_1_1();
        let mut save = Save::new(42);
        assert_eq!(save.charges_left(&chapter, "1.1.2"), 3);
        assert!(!attempt(&mut save, "1.1.2", PuzzleKind::Canyon, 0.5));
        assert!(!attempt(&mut save, "1.1.2", PuzzleKind::Canyon, 1.5));
        assert_eq!(save.charges_left(&chapter, "1.1.2"), 1);
        assert_eq!(save.progress("1.1.2").quality(), None);
        assert!(attempt(&mut save, "1.1.2", PuzzleKind::Canyon, 1.0));
        assert_eq!(save.charges_left(&chapter, "1.1.2"), 0);
        assert_eq!(save.progress("1.1.2").quality(), Some(Quality::Serviceable));
        save.recharge("1.1.2");
        assert_eq!(save.charges_left(&chapter, "1.1.2"), 3);

        assert!(attempt(&mut save, "1.1.1", PuzzleKind::Gravity, 1.0));
        assert_eq!(save.progress("1.1.1").quality(), Some(Quality::Pristine));
    }

    #[test]
    fn save_round_trips() {
        let mut save = Save::new(9);
        save.notebook.set("g", "gravity", Quantity::acceleration(8.25), "calculated");
        save.placed.insert("1.1.2".into(), 4);
        attempt(&mut save, "1.1.1", PuzzleKind::Gravity, 1.0);
        let text = save.to_ron();
        assert_eq!(Save::from_ron(&text).unwrap(), save);
        assert!(Save::from_ron(&text.replace("version: 1", "version: 99")).is_err());
    }

    #[test]
    fn saves_from_before_pages_and_demonstrations_still_load() {
        let old = r#"(version: 1, seed: 5, notebook: (entries: []), puzzles: {"1.1.0": (attempts: [], charges_used: 1, solved: true)})"#;
        let save = Save::from_ron(old).unwrap();
        assert!(save.found.is_empty() && save.demos.is_empty() && save.placed.is_empty());
        // The tutorial is done, so its page is there; the drop tower's is not yet.
        let chapter = chapter_1_1();
        assert!(save.is_found(&chapter, "units"));
        assert!(!save.is_found(&chapter, "free_fall"));
    }

    #[test]
    fn pages_and_demonstrations_are_kept() {
        let chapter = chapter_1_1();
        let mut save = Save::new(3);
        assert!(save.find("free_fall"));
        assert!(!save.find("free_fall"), "a page is written once");
        assert!(save.is_found(&chapter, "free_fall"));

        assert!(!save.demo_watched("1.1.2"));
        save.record_demo("1.1.2", Some(0));
        save.record_demo("1.1.2", Some(2));
        assert!(save.demo_watched("1.1.2"));
        assert_eq!(save.first_guess("1.1.2"), Some(0), "only the guess before the first run counts");
        save.record_demo("1.1.3", None);
        assert!(save.demo_watched("1.1.3"));
        assert_eq!(save.first_guess("1.1.3"), None);

        assert_eq!(Save::from_ron(&save.to_ron()).unwrap(), save);
    }
}
