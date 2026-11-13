//! Chapter content: the words of the curriculum, loaded from `assets/curriculum`.

use crate::puzzle::PuzzleKind;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Domain {
    Mechanics,
    Electromagnetism,
    Thermodynamics,
    Integrated,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Concept {
    pub id: String,
    pub title: String,
    /// Handbook text: definitions and relations, never a worked answer.
    pub card: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PuzzleDef {
    pub id: String,
    pub kind: PuzzleKind,
    pub title: String,
    /// One line for the objective display.
    pub task: String,
    pub brief: String,
    /// How to take the measurements this puzzle needs.
    pub investigate: String,
    /// Ids of the concepts this puzzle exercises.
    pub concepts: Vec<String>,
    /// Ids of puzzles that must be solved first.
    pub requires: Vec<String>,
    /// Prototype charges: tests available before the station must recharge.
    pub charges: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub tier: u8,
    pub domain: Domain,
    pub title: String,
    /// The ship part this chapter's capstone installs.
    pub part: String,
    pub intro: String,
    pub concepts: Vec<Concept>,
    pub puzzles: Vec<PuzzleDef>,
}

impl Chapter {
    pub fn from_ron(source: &str) -> Result<Chapter, ron::error::SpannedError> {
        ron::from_str(source)
    }

    pub fn puzzle(&self, id: &str) -> Option<&PuzzleDef> {
        self.puzzles.iter().find(|p| p.id == id)
    }

    pub fn concept(&self, id: &str) -> Option<&Concept> {
        self.concepts.iter().find(|c| c.id == id)
    }

    /// Check internal references. Returns a description of each problem found.
    pub fn problems(&self) -> Vec<String> {
        let mut problems = Vec::new();
        for (i, puzzle) in self.puzzles.iter().enumerate() {
            if self.puzzles.iter().filter(|p| p.id == puzzle.id).count() > 1 {
                problems.push(format!("puzzle id {} is used more than once", puzzle.id));
            }
            for concept in &puzzle.concepts {
                if self.concept(concept).is_none() {
                    problems.push(format!("puzzle {} names unknown concept {concept}", puzzle.id));
                }
            }
            for required in &puzzle.requires {
                if !self.puzzles[..i].iter().any(|p| &p.id == required) {
                    problems.push(format!("puzzle {} requires {required}, which does not come before it", puzzle.id));
                }
            }
            if puzzle.charges == 0 {
                problems.push(format!("puzzle {} has no charges", puzzle.id));
            }
        }
        problems
    }
}

/// Tier 1, chapter 1: The Salvage Launcher.
pub fn chapter_1_1() -> Chapter {
    Chapter::from_ron(include_str!("../../../assets/curriculum/tier1_ch1.ron"))
        .expect("assets/curriculum/tier1_ch1.ron is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_1_1_loads_and_is_consistent() {
        let chapter = chapter_1_1();
        assert_eq!(chapter.problems(), Vec::<String>::new());
        assert_eq!(chapter.puzzles.len(), 3);
        assert_eq!(
            chapter.puzzles.iter().map(|p| p.kind).collect::<Vec<_>>(),
            [PuzzleKind::Gravity, PuzzleKind::Canyon, PuzzleKind::Ridge]
        );
    }

    #[test]
    fn concept_cards_do_not_contain_worked_numbers_for_g() {
        // The handbook must not state a value for gravity: measuring it is the first puzzle.
        for concept in chapter_1_1().concepts {
            assert!(!concept.card.contains("9.8"), "{} mentions Earth's g", concept.id);
        }
    }
}
