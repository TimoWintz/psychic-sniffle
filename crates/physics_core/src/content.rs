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

/// What writes a concept's page into the handbook. A page is locked until then.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Found {
    /// Watching the demonstration at this puzzle's station.
    Demo(String),
    /// Committing a value (tested or refused) at this puzzle's station for the first time.
    FirstCommit(String),
}

impl Found {
    /// Id of the puzzle whose station writes the page.
    pub fn puzzle(&self) -> &str {
        match self {
            Found::Demo(id) | Found::FirstCommit(id) => id,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Concept {
    pub id: String,
    /// Spoken by Lumen when the page is found, so it holds no digits.
    pub title: String,
    /// Handbook text: definitions and relations, never a worked answer.
    pub card: String,
    pub found: Found,
    /// Where to find the page, shown while it is locked.
    pub hint: String,
    /// A moving picture of the idea, in a scene from Earth: never one of the island's own setups.
    #[serde(default)]
    pub illustration: Option<Illustration>,
    /// Optional questions to check the page has sunk in. They change nothing in the world.
    #[serde(default)]
    pub quiz: Vec<QuizQuestion>,
}

/// The scenes a handbook page can show, all from Earth. What each one draws lives in the game.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Scene {
    /// A tape measure along a sofa: a bare number beside a number with its unit.
    Tape,
    /// A cyclist at a steady pace past distance posts.
    Cyclist,
    /// A car catching up with a bus on a road, the gap between them marked.
    Overtake,
    /// A car pulling away from traffic lights, speeding up steadily.
    GreenLight,
    /// An apple dropped from a balcony, beside a feather and a hammer in a vacuum tube.
    AppleDrop,
    /// One marble rolled off a table and one dropped from its edge.
    TableEdge,
    /// A football kick with its velocity split into its sideways and upward parts.
    KickSplit,
    /// Two kicks at the same speed and complementary angles.
    TwinKicks,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Illustration {
    pub scene: Scene,
    /// One or two lines under the picture.
    pub caption: String,
}

/// A handbook page's question. Shown, never spoken, so it may hold numbers (Earth's, never the island's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct QuizQuestion {
    pub question: String,
    pub choices: Vec<String>,
    /// Index of the right choice.
    pub answer: usize,
    /// Why, shown once a choice is made.
    pub explain: String,
}

/// A station's demonstration: a question the player answers by choosing, before watching.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DemoDef {
    pub question: String,
    pub choices: Vec<String>,
    /// Index of the choice that matches what happens.
    pub answer: usize,
    /// What the world showed, in words. Spoken by Lumen, so it holds no digits.
    pub reveal: String,
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
    /// The free experiment to try before any calculation, if the station has one.
    #[serde(default)]
    pub demo: Option<DemoDef>,
    /// Why this job matters to the story, in Lumen's voice: said when the station first opens and
    /// shown at the top of its panel. Spoken, so it holds no digits.
    #[serde(default)]
    pub why: String,
    /// Lumen's line when the puzzle is passed: what it got us, and what it leads to. Spoken.
    #[serde(default)]
    pub done: String,
    /// The caption over the fade when the castaway wires this station's console into its machine,
    /// once the light has woken it. The first station's console is in from the start.
    #[serde(default)]
    pub rigged: String,
}

/// A line Lumen says, with a place its eye holds while it says it (a landmark's name).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spoken {
    pub text: String,
    #[serde(default)]
    pub look: Option<String>,
}

/// When a remark may be made, by the state of a puzzle.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemarkWhen {
    Always,
    /// While this puzzle is not solved.
    Before(String),
    /// Once this puzzle is solved.
    After(String),
}

/// Something Lumen says once, unprompted, when the player comes near a place and nothing else is
/// going on.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Remark {
    /// Name of the landmark the player comes near.
    pub near: String,
    /// How near, in metres.
    pub radius: f32,
    pub when: RemarkWhen,
    pub line: Spoken,
}

impl Remark {
    /// Ids of the puzzles named by the remark's condition.
    fn puzzle(&self) -> Option<&str> {
        match &self.when {
            RemarkWhen::Always => None,
            RemarkWhen::Before(id) | RemarkWhen::After(id) => Some(id),
        }
    }
}

/// A tier: the ship it builds and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tier {
    pub number: u8,
    /// The ship this tier builds.
    pub ship: String,
    /// One line: what the tier is for in the story.
    pub goal: String,
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
    /// One line for the dock: what the chapter is for in the story.
    #[serde(default)]
    pub goal: String,
    /// Lumen's first lines in a fresh game.
    #[serde(default)]
    pub opening: Vec<Spoken>,
    /// Lumen's lines when the chapter's last puzzle is passed.
    #[serde(default)]
    pub closing: Vec<Spoken>,
    /// Lumen's remarks about places, each said once.
    #[serde(default)]
    pub remarks: Vec<Remark>,
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

    /// The handbook pages a puzzle's station writes, in chapter order.
    pub fn pages_found_at(&self, puzzle_id: &str) -> Vec<&Concept> {
        self.concepts.iter().filter(|c| c.found.puzzle() == puzzle_id).collect()
    }

    fn position(&self, puzzle_id: &str) -> Option<usize> {
        self.puzzles.iter().position(|p| p.id == puzzle_id)
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
            // Every page a puzzle needs can be found at its own station or an earlier one.
            for id in &puzzle.concepts {
                if let Some(concept) = self.concept(id)
                    && self.position(concept.found.puzzle()).is_none_or(|at| at > i)
                {
                    problems.push(format!("puzzle {} needs page {id}, which is found only later", puzzle.id));
                }
            }
            if puzzle.why.trim().is_empty() || puzzle.done.trim().is_empty() {
                problems.push(format!("puzzle {}: Lumen needs a why and a done line", puzzle.id));
            }
            if has_digit(&puzzle.why) || has_digit(&puzzle.done) {
                problems.push(format!("puzzle {}: the why and done lines are spoken and must hold no digits", puzzle.id));
            }
            if i > 0 && puzzle.rigged.trim().is_empty() {
                problems.push(format!("puzzle {}: its console needs a caption for when it is wired in", puzzle.id));
            }
            if has_digit(&puzzle.rigged) {
                problems.push(format!("puzzle {}: the wiring caption must hold no digits", puzzle.id));
            }
            if let Some(demo) = &puzzle.demo {
                if demo.choices.len() < 2 || demo.answer >= demo.choices.len() {
                    problems.push(format!("puzzle {}: the demonstration needs choices and an answer among them", puzzle.id));
                }
                if has_digit(&demo.reveal) {
                    problems.push(format!("puzzle {}: the demonstration's reveal is spoken and must hold no digits", puzzle.id));
                }
                if !self.concepts.iter().any(|c| c.found == Found::Demo(puzzle.id.clone())) {
                    problems.push(format!("puzzle {}: the demonstration writes no page", puzzle.id));
                }
            }
        }
        for concept in &self.concepts {
            match self.puzzle(concept.found.puzzle()) {
                None => problems.push(format!("page {} is found at unknown puzzle {}", concept.id, concept.found.puzzle())),
                Some(puzzle) if matches!(concept.found, Found::Demo(_)) && puzzle.demo.is_none() => {
                    problems.push(format!("page {} is found by a demonstration puzzle {} does not have", concept.id, puzzle.id));
                }
                Some(_) => {}
            }
            if has_digit(&concept.title) {
                problems.push(format!("page {}: the title is spoken and must hold no digits", concept.id));
            }
            for (n, question) in concept.quiz.iter().enumerate() {
                if question.choices.len() < 2 || question.answer >= question.choices.len() {
                    problems.push(format!("page {}: quiz question {} needs choices and an answer among them", concept.id, n + 1));
                }
                if question.explain.trim().is_empty() {
                    problems.push(format!("page {}: quiz question {} needs an explanation", concept.id, n + 1));
                }
            }
        }
        let spoken = self.opening.iter().chain(&self.closing).chain(self.remarks.iter().map(|r| &r.line));
        for line in spoken {
            if has_digit(&line.text) {
                problems.push(format!("Lumen's line \"{}\" must hold no digits", line.text));
            }
        }
        for remark in &self.remarks {
            if let Some(id) = remark.puzzle()
                && self.puzzle(id).is_none()
            {
                problems.push(format!("a remark at {} names unknown puzzle {id}", remark.near));
            }
        }
        problems
    }

    /// Every landmark the chapter's lines name, for the game to check against its places.
    pub fn places_named(&self) -> Vec<&str> {
        let looks = self.opening.iter().chain(&self.closing).chain(self.remarks.iter().map(|r| &r.line)).filter_map(|l| l.look.as_deref());
        looks.chain(self.remarks.iter().map(|r| r.near.as_str())).collect()
    }
}

/// Lumen's scripted lines never contain a number, so they cannot give an answer away.
fn has_digit(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit())
}

/// Tier 1: the skiff.
pub fn tier_1() -> Tier {
    ron::from_str(include_str!("../../../assets/curriculum/tier1.ron")).expect("assets/curriculum/tier1.ron is valid")
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
        assert_eq!(chapter.puzzles.len(), 5);
        assert_eq!(
            chapter.puzzles.iter().map(|p| p.kind).collect::<Vec<_>>(),
            [PuzzleKind::Rail, PuzzleKind::Chase, PuzzleKind::Gravity, PuzzleKind::Canyon, PuzzleKind::Ridge]
        );
        // The tutorial comes first and everything else waits for it.
        assert_eq!(chapter.puzzles[0].id, "1.1.0");
        assert!(chapter.puzzles[0].requires.is_empty());
        assert_eq!(chapter.puzzles[1].requires, ["1.1.0"]);
        // Velocity comes before acceleration: the drop tower waits for the haul track.
        assert_eq!(chapter.puzzles[2].requires, ["1.1.1"]);
        // The story: a goal, an opening and a closing, and something to say about places.
        assert!(!chapter.goal.is_empty() && !chapter.opening.is_empty() && !chapter.closing.is_empty());
        assert!(!chapter.remarks.is_empty());
    }

    #[test]
    fn tier_1_loads() {
        let tier = tier_1();
        assert_eq!(tier.number, chapter_1_1().tier);
        assert!(!tier.goal.is_empty() && !has_digit(&tier.goal));
    }

    #[test]
    fn every_page_is_found_somewhere_in_the_world() {
        let chapter = chapter_1_1();
        // The tutorial writes the first page, so the handbook is met before any physics.
        assert_eq!(chapter.concept("units").unwrap().found, Found::FirstCommit("1.1.0".into()));
        assert!(chapter.puzzles[0].demo.is_none());
        for puzzle in &chapter.puzzles[1..] {
            assert!(puzzle.demo.is_some(), "{} has a demonstration", puzzle.id);
            assert!(!chapter.pages_found_at(&puzzle.id).is_empty(), "{} writes a page", puzzle.id);
        }
        let chase: Vec<&str> = chapter.pages_found_at("1.1.1").iter().map(|c| c.id.as_str()).collect();
        assert_eq!(chase, ["velocity", "relative_velocity"]);
        // Acceleration is defined before free fall uses it; the drop tower writes both.
        let gravity: Vec<&str> = chapter.pages_found_at("1.1.2").iter().map(|c| c.id.as_str()).collect();
        assert_eq!(gravity, ["acceleration", "free_fall"]);
        let ridge: Vec<&str> = chapter.pages_found_at("1.1.4").iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ridge, ["components", "range"]);
    }

    #[test]
    fn problems_are_caught() {
        let mut chapter = chapter_1_1();
        chapter.concepts[1].found = Found::Demo("1.1.4".into());
        chapter.concepts[2].title = "Range at 45 degrees".into();
        chapter.puzzles[2].demo.as_mut().unwrap().answer = 7;
        chapter.puzzles[3].done = String::new();
        chapter.puzzles[4].why = "Clear the ridge at 45 degrees.".into();
        chapter.puzzles[1].rigged = String::new();
        chapter.puzzles[2].rigged = "By evening, 2 consoles.".into();
        chapter.remarks[0].line.text = "Only 3 more.".into();
        chapter.remarks[1].when = RemarkWhen::After("1.1.9".into());
        chapter.concepts[3].quiz[0].answer = 9;
        chapter.concepts[4].quiz[1].explain = " ".into();
        let problems = chapter.problems();
        assert!(problems.iter().any(|p| p.contains("found only later")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("must hold no digits")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("an answer among them")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("1.1.3: Lumen needs a why and a done line")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("1.1.4: the why and done lines")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("1.1.1: its console needs a caption")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("1.1.2: the wiring caption must hold no digits")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("Only 3 more.")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("unknown puzzle 1.1.9")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("quiz question 1 needs choices")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("quiz question 2 needs an explanation")), "{problems:?}");
    }

    #[test]
    fn every_page_has_a_picture_and_a_short_quiz() {
        let chapter = chapter_1_1();
        for concept in &chapter.concepts {
            assert!(concept.illustration.is_some(), "{} has an illustration", concept.id);
            assert!((2..=3).contains(&concept.quiz.len()), "{} has two or three questions", concept.id);
        }
        // Each scene is drawn once: a page's picture is its own.
        let scenes: Vec<Scene> = chapter.concepts.iter().filter_map(|c| c.illustration.as_ref().map(|i| i.scene)).collect();
        for (i, scene) in scenes.iter().enumerate() {
            assert!(!scenes[..i].contains(scene), "{scene:?} is drawn on two pages");
        }
    }

    #[test]
    fn concept_cards_do_not_contain_worked_numbers_for_g() {
        // The handbook must not state a value for gravity: measuring it is the first puzzle.
        for concept in chapter_1_1().concepts {
            assert!(!concept.card.contains("9.8"), "{} mentions Earth's g", concept.id);
        }
    }
}
