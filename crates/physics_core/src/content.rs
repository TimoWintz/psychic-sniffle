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

/// The light a chapter is played in. Each is a preset the game draws (the sun, the sky, the haze), so
/// a chapter chooses a mood and the look is tuned in one place. More come with the chapters that
/// need them: the crossing runs into the night.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Light {
    /// Late afternoon: a low, warm sun and long shadows under a turquoise sky.
    Afternoon,
}

/// What writes a concept's page into the handbook. A page is locked until then.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Found {
    /// Watching the demonstration at this puzzle's station.
    Demo(String),
    /// Committing a value (tested or refused) at this puzzle's station for the first time.
    FirstCommit(String),
    /// Waking this side-machine: its job done right, every case of it.
    Side(String),
}

impl Found {
    /// Id of the puzzle whose station writes the page, or of the side-machine that does.
    pub fn puzzle(&self) -> &str {
        match self {
            Found::Demo(id) | Found::FirstCommit(id) | Found::Side(id) => id,
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
    /// A ball dropped on a floor, each bounce a set fraction of the last, the next one predicted.
    Bounce,
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
}

/// What kind of side-machine it is: how it is built and judged (game code), as a puzzle's kind is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SideKind {
    /// A beam on a pivot, balanced with blocks (see `beam`).
    Beam,
}

/// A side-machine of the builders off the road: optional, answered by moving things, never by a
/// typed value (see "Side-machines" in the game design). It writes a page about how science is
/// done when it wakes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SideDef {
    pub id: String,
    pub kind: SideKind,
    /// The name its console shows.
    pub title: String,
    /// What to do, as plain instructions.
    pub brief: String,
    /// Lumen's line when its console is first used. Spoken, so it holds no digits.
    pub why: String,
    /// Lumen's line when it wakes. Spoken.
    pub done: String,
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
    /// While this puzzle is not solved (or this side-machine not woken).
    Before(String),
    /// Once this puzzle is solved (or this side-machine woken).
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

/// A tier: the vessel it wakes and why.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Tier {
    pub number: u8,
    /// The vessel this tier wakes and takes across.
    pub ship: String,
    /// One line: what the tier is for in the story.
    pub goal: String,
}

impl Tier {
    /// The tier's words against the rules in `docs/writing.md`, as [`Chapter::style_problems`] does.
    pub fn style_problems(&self) -> Vec<String> {
        text_style(&self.goal).into_iter().map(|p| format!("tier {} goal: {p}", self.number)).collect()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    pub id: String,
    pub tier: u8,
    pub domain: Domain,
    pub title: String,
    /// The part of the vessel this chapter's capstone wakes.
    pub part: String,
    /// The light the chapter is played in.
    pub light: Light,
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
    /// The builders' side-machines off the road, if the chapter has any.
    #[serde(default)]
    pub sides: Vec<SideDef>,
}

impl Chapter {
    pub fn from_ron(source: &str) -> Result<Chapter, ron::error::SpannedError> {
        ron::from_str(source)
    }

    pub fn puzzle(&self, id: &str) -> Option<&PuzzleDef> {
        self.puzzles.iter().find(|p| p.id == id)
    }

    pub fn side(&self, id: &str) -> Option<&SideDef> {
        self.sides.iter().find(|s| s.id == id)
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
        for side in &self.sides {
            if self.puzzle(&side.id).is_some() || self.sides.iter().filter(|s| s.id == side.id).count() > 1 {
                problems.push(format!("side-machine id {} is used more than once", side.id));
            }
            if side.why.trim().is_empty() || side.done.trim().is_empty() {
                problems.push(format!("side-machine {}: Lumen needs a why and a done line", side.id));
            }
            if has_digit(&side.why) || has_digit(&side.done) {
                problems.push(format!("side-machine {}: the why and done lines are spoken and must hold no digits", side.id));
            }
            if !self.concepts.iter().any(|c| c.found == Found::Side(side.id.clone())) {
                problems.push(format!("side-machine {} writes no page", side.id));
            }
        }
        for concept in &self.concepts {
            if let Found::Side(id) = &concept.found {
                if self.side(id).is_none() {
                    problems.push(format!("page {} is found at unknown side-machine {id}", concept.id));
                }
                continue;
            }
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
                && self.side(id).is_none()
            {
                problems.push(format!("a remark at {} names unknown puzzle {id}", remark.near));
            }
        }
        problems
    }

    /// Lines that break the writing rules in `docs/writing.md` that a program can check. These are
    /// kept apart from [`Chapter::problems`], so a slip of style never stops a chapter from loading.
    /// Tests require every chapter in `assets/curriculum` to have none.
    pub fn style_problems(&self) -> Vec<String> {
        let mut spoken: Vec<(String, &str)> = Vec::new();
        for line in &self.opening {
            spoken.push(("opening".into(), &line.text));
        }
        for line in &self.closing {
            spoken.push(("closing".into(), &line.text));
        }
        for remark in &self.remarks {
            spoken.push((format!("remark at {}", remark.near), &remark.line.text));
        }
        for puzzle in &self.puzzles {
            spoken.push((format!("puzzle {} why", puzzle.id), &puzzle.why));
            spoken.push((format!("puzzle {} done", puzzle.id), &puzzle.done));
            if let Some(demo) = &puzzle.demo {
                spoken.push((format!("puzzle {} reveal", puzzle.id), &demo.reveal));
            }
        }
        for side in &self.sides {
            spoken.push((format!("side-machine {} why", side.id), &side.why));
            spoken.push((format!("side-machine {} done", side.id), &side.done));
        }

        let mut shown: Vec<(String, &str)> = vec![("intro".into(), &self.intro), ("goal".into(), &self.goal)];
        for concept in &self.concepts {
            shown.push((format!("page {} title", concept.id), &concept.title));
            shown.push((format!("page {} card", concept.id), &concept.card));
            shown.push((format!("page {} hint", concept.id), &concept.hint));
            if let Some(illustration) = &concept.illustration {
                shown.push((format!("page {} caption", concept.id), &illustration.caption));
            }
            for (n, question) in concept.quiz.iter().enumerate() {
                shown.push((format!("page {} quiz question {}", concept.id, n + 1), &question.question));
                shown.push((format!("page {} quiz explanation {}", concept.id, n + 1), &question.explain));
                for choice in &question.choices {
                    shown.push((format!("page {} quiz choice", concept.id), choice));
                }
            }
        }
        for puzzle in &self.puzzles {
            shown.push((format!("puzzle {} task", puzzle.id), &puzzle.task));
            shown.push((format!("puzzle {} brief", puzzle.id), &puzzle.brief));
            shown.push((format!("puzzle {} investigate", puzzle.id), &puzzle.investigate));
            if let Some(demo) = &puzzle.demo {
                shown.push((format!("puzzle {} demo question", puzzle.id), &demo.question));
                for choice in &demo.choices {
                    shown.push((format!("puzzle {} demo choice", puzzle.id), choice));
                }
            }
        }

        for side in &self.sides {
            shown.push((format!("side-machine {} title", side.id), &side.title));
            shown.push((format!("side-machine {} brief", side.id), &side.brief));
        }

        let mut problems = Vec::new();
        for puzzle in &self.puzzles {
            problems.extend(task_style(&puzzle.task).into_iter().map(|p| format!("puzzle {} task: {p}", puzzle.id)));
        }
        for (place, text) in &spoken {
            problems.extend(spoken_style(text).into_iter().map(|p| format!("{place}: {p}")));
        }
        for (place, text) in spoken.iter().chain(&shown) {
            problems.extend(text_style(text).into_iter().map(|p| format!("{place}: {p}")));
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

/// Phrases Lumen says as one word: it talks like a person, with contractions. See `docs/writing.md`.
const UNCONTRACTED: [&str; 21] = [
    "i am", "i have not", "i do not", "i did not", "i will", "i would", "do not", "does not", "did not", "is not", "are not", "was not", "cannot", "can not",
    "will not", "would not", "it is", "that is", "we are", "you are", "let us",
];

/// Words and phrases that mark text as machine-written, wherever they appear. See `docs/writing.md`.
const STOCK: [&str; 18] = [
    "delve", "tapestry", "testament", "crucial", "pivotal", "seamless", "seamlessly", "vibrant", "showcase", "underscore", "embark", "journey", "realm",
    "moreover", "furthermore", "additionally", "not just", "it's worth",
];

/// The longest line Lumen says, in words: its lines play in a bubble, one after another.
const LONGEST_SPOKEN: usize = 60;

/// The text's words, lower case, with apostrophes kept inside them.
fn words(text: &str) -> Vec<String> {
    text.replace('’', "'").split(|c: char| !(c.is_alphanumeric() || c == '\'')).map(|w| w.trim_matches('\'').to_lowercase()).filter(|w| !w.is_empty()).collect()
}

/// Whether the words of `phrase` appear in a row among `words`.
fn has_phrase(words: &[String], phrase: &str) -> bool {
    let wanted: Vec<&str> = phrase.split(' ').collect();
    words.windows(wanted.len()).any(|w| w.iter().zip(&wanted).all(|(a, b)| a == b))
}

/// What breaks the rules for a line Lumen says.
fn spoken_style(text: &str) -> Vec<String> {
    let found = words(text);
    let mut problems: Vec<String> = UNCONTRACTED.iter().filter(|p| has_phrase(&found, p)).map(|p| format!("\"{p}\" should be a contraction")).collect();
    if text.matches(':').count() > 1 {
        problems.push("more than one colon in a spoken line".into());
    }
    if found.len() > LONGEST_SPOKEN {
        problems.push(format!("{} words is too long for one spoken line", found.len()));
    }
    problems
}

/// What breaks the rules for any text the player reads or hears.
fn text_style(text: &str) -> Vec<String> {
    let found = words(text);
    let mut problems: Vec<String> = STOCK.iter().filter(|p| has_phrase(&found, p)).map(|p| format!("stock phrase \"{p}\"")).collect();
    if text.contains('—') || text.contains('–') || text.contains(" -- ") {
        problems.push("a dash: use a comma, a full stop or brackets".into());
    }
    problems
}

/// Words an imperative sentence can't start with. A program can't tell a verb from any other word,
/// but a task that starts with one of these isn't an instruction.
const NOT_IMPERATIVE: [&str; 24] = [
    "the", "a", "an", "this", "that", "these", "those", "we", "we'll", "we're", "you", "you'll", "i", "it", "it's", "they", "our", "your", "there", "here",
    "now", "next", "first", "then",
];

/// What breaks the rules for a puzzle's `task`: one imperative sentence that ends with a full stop,
/// since Lumen reads it inside a sentence of its own ("Next we need to load the rail cart...").
fn task_style(task: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let task = task.trim();
    if !task.ends_with('.') {
        problems.push("a task ends with a full stop".to_string());
    }
    let body = task.strip_suffix('.').unwrap_or(task);
    if body.contains(['.', '?', '!', ';', ':']) {
        problems.push("a task is one sentence".to_string());
    }
    if !task.starts_with(|c: char| c.is_uppercase()) {
        problems.push("a task starts with a capital letter".to_string());
    }
    if let Some(first) = words(task).first()
        && NOT_IMPERATIVE.contains(&first.as_str())
    {
        problems.push(format!("a task starts with a verb, not \"{first}\""));
    }
    problems
}

/// Tier 1: the skiff.
pub fn tier_1() -> Tier {
    ron::from_str(include_str!("../../../assets/curriculum/tier1.ron")).expect("assets/curriculum/tier1.ron is valid")
}

/// Tier 1, chapter 1: The Helm.
pub fn chapter_1_1() -> Chapter {
    Chapter::from_ron(include_str!("../../../assets/curriculum/tier1_ch1.ron")).expect("assets/curriculum/tier1_ch1.ron is valid")
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
            [PuzzleKind::Rail, PuzzleKind::Chase, PuzzleKind::Gravity, PuzzleKind::Channel, PuzzleKind::Ridge]
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
    fn the_beam_is_a_side_machine_with_a_method_page() {
        let chapter = chapter_1_1();
        assert_eq!(chapter.sides.len(), 1);
        let beam = &chapter.sides[0];
        assert_eq!((beam.id.as_str(), beam.kind), ("1.1.s1", SideKind::Beam));
        assert!(chapter.puzzle(&beam.id).is_none(), "a side-machine is not one of the chapter's puzzles");
        let pages: Vec<&str> = chapter.pages_found_at(&beam.id).iter().map(|c| c.id.as_str()).collect();
        assert_eq!(pages, ["a_rule_predicts"]);
        // No puzzle needs its page: the main route never waits for a side-track.
        for puzzle in &chapter.puzzles {
            assert!(!puzzle.concepts.iter().any(|c| c == "a_rule_predicts"), "{} needs the beam's page", puzzle.id);
        }
        // Lumen remarks on it, asleep and awake.
        let remarks: Vec<&RemarkWhen> = chapter.remarks.iter().map(|r| &r.when).collect();
        assert!(remarks.contains(&&RemarkWhen::Before(beam.id.clone())) && remarks.contains(&&RemarkWhen::After(beam.id.clone())));

        let mut broken = chapter.clone();
        broken.sides[0].done = "Done in 3.".into();
        broken.concepts.last_mut().unwrap().found = Found::Side("1.1.s9".into());
        broken.puzzles[1].concepts.push("a_rule_predicts".into());
        let problems = broken.problems();
        assert!(problems.iter().any(|p| p.contains("side-machine 1.1.s1: the why and done lines")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("unknown side-machine 1.1.s9")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("side-machine 1.1.s1 writes no page")), "{problems:?}");
        let mut needs = chapter.clone();
        needs.puzzles[1].concepts.push("a_rule_predicts".into());
        assert!(needs.problems().iter().any(|p| p.contains("1.1.1 needs page a_rule_predicts")), "{:?}", needs.problems());
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
        assert!(problems.iter().any(|p| p.contains("Only 3 more.")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("unknown puzzle 1.1.9")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("quiz question 1 needs choices")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("quiz question 2 needs an explanation")), "{problems:?}");
    }

    /// Every `.ron` file in `assets/curriculum` whose name passes `keep`, with its source, sorted by
    /// name, so a new chapter or tier is checked as soon as it is added.
    fn curriculum_files(keep: impl Fn(&str) -> bool) -> Vec<(String, String)> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/curriculum");
        let mut files: Vec<(String, String)> = std::fs::read_dir(&dir)
            .expect("assets/curriculum can be read")
            .map(|entry| entry.expect("a directory entry").path())
            .filter_map(|path| {
                let name = path.file_name()?.to_str()?.to_string();
                (name.ends_with(".ron") && keep(&name)).then(|| (name, std::fs::read_to_string(&path).expect("a curriculum file can be read")))
            })
            .collect();
        files.sort();
        assert!(!files.is_empty(), "no matching files found in {}", dir.display());
        files
    }

    #[test]
    fn every_chapter_follows_the_writing_rules() {
        // docs/writing.md: the rules a program can check. The rest is for whoever writes the lines.
        for (name, source) in curriculum_files(|n| n.contains("_ch")) {
            let chapter = Chapter::from_ron(&source).unwrap_or_else(|e| panic!("{name} is valid: {e}"));
            assert_eq!(chapter.style_problems(), Vec::<String>::new(), "{name} breaks docs/writing.md");
        }
    }

    #[test]
    fn every_tier_follows_the_writing_rules() {
        for (name, source) in curriculum_files(|n| n.starts_with("tier") && !n.contains("_ch")) {
            let tier: Tier = ron::from_str(&source).unwrap_or_else(|e| panic!("{name} is valid: {e}"));
            assert_eq!(tier.style_problems(), Vec::<String>::new(), "{name} breaks docs/writing.md");
        }
        let mut tier = tier_1();
        tier.goal = "Embark on a journey across the strait.".into();
        assert_eq!(tier.style_problems().len(), 2, "{:?}", tier.style_problems());
    }

    #[test]
    fn a_chapter_names_a_light_the_game_can_draw() {
        assert_eq!(chapter_1_1().light, Light::Afternoon);
        let source = include_str!("../../../assets/curriculum/tier1_ch1.ron");
        assert!(Chapter::from_ron(&source.replace("light: Afternoon", "light: Teatime")).is_err(), "an unknown light is refused");
        assert!(Chapter::from_ron(&source.replace("    light: Afternoon,", "")).is_err(), "every chapter chooses its light");
    }

    #[test]
    fn style_slips_are_caught() {
        let mut chapter = chapter_1_1();
        chapter.puzzles[0].why = "That is the cart. I do not know who built it.".into();
        chapter.puzzles[1].done = "The sight: lifted. The crane: awake. The tower: next.".into();
        chapter.remarks[0].line.text = "word ".repeat(LONGEST_SPOKEN + 1);
        chapter.concepts[0].card = "Units — a crucial idea.".into();
        chapter.puzzles[2].brief = "Let's delve into the tower.".into();
        let problems = chapter.style_problems();
        assert!(problems.iter().any(|p| p.contains("1.1.0 why: \"i do not\"")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("1.1.0 why: \"that is\"")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("1.1.1 done: more than one colon")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("too long for one spoken line")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("page units card: stock phrase \"crucial\"")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("page units card: a dash")), "{problems:?}");
        assert!(problems.iter().any(|p| p.contains("1.1.2 brief: stock phrase \"delve\"")), "{problems:?}");
        // Contractions are only asked of Lumen: the handbook and the briefs may spell words out.
        chapter.concepts[1].card = "It is a speed, and that is all.".into();
        assert!(!chapter.style_problems().iter().any(|p| p.contains("page velocity")));
        // A word that only contains a stock word is fine.
        assert!(text_style("The journeyman's showcases").is_empty());
    }

    #[test]
    fn a_task_is_one_instruction_with_a_full_stop() {
        assert!(task_style("Load the rail cart and send it to the orange marker.").is_empty());
        assert!(task_style("Measure this planet's gravitational acceleration.").is_empty());
        assert_eq!(task_style("Load the rail cart"), ["a task ends with a full stop"]);
        assert_eq!(task_style("Load the cart. Then send it."), ["a task is one sentence"]);
        assert_eq!(task_style("load the cart."), ["a task starts with a capital letter"]);
        assert_eq!(task_style("The cart needs loading."), ["a task starts with a verb, not \"the\""]);
        assert_eq!(task_style("We'll load the cart."), ["a task starts with a verb, not \"we'll\""]);
        let mut chapter = chapter_1_1();
        chapter.puzzles[3].task = "The casket goes across the channel".into();
        let problems = chapter.style_problems();
        assert!(problems.iter().any(|p| p == "puzzle 1.1.3 task: a task ends with a full stop"), "{problems:?}");
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
        // The handbook must not state a value for gravity: measuring it is a puzzle (1.1.2).
        for concept in chapter_1_1().concepts {
            assert!(!concept.card.contains("9.8"), "{} mentions Earth's g", concept.id);
        }
    }
}
