//! The snapshot handed to the tutor: a plain copy of what it is allowed to know.
//!
//! This is the only thing that crosses from the game to the tutor. It is data; it holds no handle to
//! anything in the game.

use physics_core::content::{Chapter, PuzzleDef};
use physics_core::progress::Save;
use physics_core::puzzle::{InputSource, Instance, PuzzleKind};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConceptCard {
    pub title: String,
    pub card: String,
    /// Whether the page is in the player's handbook yet (pages are found by watching experiments).
    pub found: bool,
    /// Where the player can find the page while it is locked.
    pub hint: String,
}

/// The station's free experiment, and what the player made of it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ExperimentLine {
    pub question: String,
    pub choices: Vec<String>,
    /// What actually happens.
    pub answer: String,
    pub watched: bool,
    /// The choice made before the first run, if any.
    pub guess: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AskedValue {
    pub label: String,
    /// "a speed (m/s)"
    pub dimension: String,
    /// What the player has typed in the blueprint so far, if anything.
    pub draft: String,
    /// For a value the player sets by building in the world rather than typing: what stands there
    /// now ("2 blocks in the jack frame").
    pub built: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct NotebookLine {
    pub name: String,
    pub label: String,
    pub value: String,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AttemptLine {
    pub committed: Vec<String>,
    pub pass: bool,
    pub headline: String,
    pub detail: String,
}

/// A numeric value the tutor must not reveal, as a range of numbers in one unit.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Secret {
    pub low: f64,
    pub high: f64,
    /// If non-empty, a number only counts when one of these words follows it ("deg", "m/s"). Used for
    /// wide ranges, where redacting every number inside would catch innocent ones. A word counts only
    /// as a whole word: "m" is not the start of "m/s" or "metres".
    pub only_before: Vec<String>,
    /// How many other words may stand between the number and the word that scopes it, for counting
    /// nouns ("5 stone blocks", "5 of the blocks").
    pub gap: usize,
    /// For an answer measured rather than counted: a whole number in the reply counts only with one of
    /// these units after it ("about 12 m/s"). The 2 in "(1/2) g t^2" or "3 charges" is left alone, even
    /// when the answer is close to 2 or 3.
    pub whole_needs: Vec<String>,
}

impl Secret {
    /// Every number from `low` to `high`, wherever it stands.
    pub fn between(low: f64, high: f64) -> Self {
        Secret { low: low.min(high), high: low.max(high), only_before: Vec::new(), gap: 0, whole_needs: Vec::new() }
    }

    /// A value and everything within [`SECRET_MARGIN`] of it. With `whole_needs`, a whole number in
    /// the reply counts only with one of those units after it; leave it empty for a value that is a
    /// count or a length in whole metres, which is guarded wherever it stands.
    pub fn near(value: f64, whole_needs: &[&str]) -> Self {
        let secret = Secret::between(value * (1.0 - SECRET_MARGIN), value * (1.0 + SECRET_MARGIN));
        Secret { whole_needs: words(whole_needs), ..secret }
    }

    /// Only where one of `words` follows the number.
    pub fn before(self, words_after: &[&str]) -> Self {
        Secret { only_before: words(words_after), ..self }
    }

    /// Allow `gap` words between the number and the word that scopes it.
    pub fn gap(self, gap: usize) -> Self {
        Secret { gap, ..self }
    }
}

fn words(list: &[&str]) -> Vec<String> {
    list.iter().map(|w| w.to_string()).collect()
}

/// How a unit is written after a number, by the unit's symbol in the puzzle ("m/s", "deg").
pub fn spellings(unit: &str) -> &'static [&'static str] {
    match unit {
        "m" => &["m", "metre", "metres", "meter", "meters"],
        "m/s" => SPEED_WORDS,
        // "m/s" covers "m/s^2" and "m/s²" too.
        "m/s^2" => &["m/s", "m s", "metres per second", "meters per second"],
        "deg" => ANGLE_WORDS,
        "" | "rad" => &["rad", "radian", "radians"],
        _ => &[],
    }
}

const SPEED_WORDS: &[&str] = &["m/s", "m s", "metres per second", "meters per second", "metre per second", "meter per second"];
const ANGLE_WORDS: &[&str] = &["deg", "degree", "degrees", "°", "º"];
const LENGTH_WORDS: &[&str] = &["m", "metre", "metres", "meter", "meters"];
const COUNTED_BLOCKS: &[&str] = &["block", "blocks", "crate", "crates", "box", "boxes"];

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TutorContext {
    /// What the tier's ship is for in the story. Filled in by the game, which knows the tier.
    pub tier_goal: String,
    pub chapter_title: String,
    /// What the chapter is for in the story.
    pub chapter_goal: String,
    /// Why this station matters to the story, as Lumen says it.
    pub why: String,
    pub puzzle_title: String,
    pub brief: String,
    pub how_to_measure: String,
    pub concepts: Vec<ConceptCard>,
    pub experiment: Option<ExperimentLine>,
    /// What the player has placed at the station in the world, in words ("the block is on the
    /// cart", "the light gate is clamped 7 m below the release point"), where something can be.
    pub placed: Option<String>,
    pub asked: Vec<AskedValue>,
    pub notebook: Vec<NotebookLine>,
    pub attempts: Vec<AttemptLine>,
    pub solved: bool,
    pub charges_left: u32,

    /// Private: the worked solution.
    pub solution_steps: Vec<String>,
    pub solution_answers: Vec<String>,
    pub solution_note: String,
    /// Private: true values of everything measurable, so the tutor can spot a mismeasurement.
    pub true_measurements: Vec<String>,

    /// Places Lumen can turn its eye to, by name. Filled in by the game, which knows where they are.
    pub landmarks: Vec<String>,

    /// Used by the leak filter, not sent to the model.
    #[serde(skip)]
    pub secrets: Vec<Secret>,
    /// Numbers the player already has; never redacted. Not sent to the model.
    #[serde(skip)]
    pub known_numbers: Vec<f64>,
}

/// Fraction either side of an answer that still counts as revealing it.
pub const SECRET_MARGIN: f64 = 0.02;

impl TutorContext {
    /// Copy what the tutor may know out of the game's state. `drafts` are the texts currently typed
    /// in the blueprint, in the order of [`Instance::inputs`].
    pub fn build(chapter: &Chapter, def: &PuzzleDef, instance: &Instance, save: &Save, drafts: &[String]) -> Self {
        let progress = save.progress(&def.id);
        let solution = instance.solution();
        let placed = instance.built(save.placed.get(&def.id).copied().unwrap_or(0));

        let mut secrets = Vec::new();
        for answer in &solution.answers {
            let mut forms = vec![(answer.quantity.value, if answer.unit == "deg" { "rad" } else { answer.unit.as_str() })];
            if let Some(unit) = physics_core::units::lookup_unit(&answer.unit)
                && unit.value != 1.0
            {
                forms.push((answer.quantity.value / unit.value, answer.unit.as_str()));
            }
            for (v, unit) in forms {
                // Lengths here are whole metres, counted in blocks: a bare whole number may be one.
                let whole_needs = if unit == "m" { &[][..] } else { spellings(unit) };
                secrets.push(Secret::near(v, whole_needs));
            }
        }
        if instance.kind == PuzzleKind::Channel {
            // One or two stacks work, each with its own speed. A stack is a small whole number, too
            // common to redact on its own: guard it where it counts blocks, and guard the release height
            // it gives, in metres. Guard each stack's speed closely, and the speeds across all of them
            // in m/s.
            secrets.clear();
            let layout = &instance.layout;
            let stacks = layout.channel_stacks();
            for stack in stacks.clone() {
                let stack_f = stack as f64;
                secrets.push(Secret::between(stack_f - 0.01, stack_f + 0.01).before(COUNTED_BLOCKS).gap(2));
                for height in [stack_f, layout.ledge_h as f64 + stack_f] {
                    secrets.push(Secret::between(height * (1.0 - SECRET_MARGIN) - 0.15, height * (1.0 + SECRET_MARGIN) + 0.15).before(LENGTH_WORDS));
                }
                secrets.push(Secret::near(layout.channel_speed_needed(stack), SPEED_WORDS));
            }
            let (fast, slow) = (layout.channel_speed_needed(*stacks.start()), layout.channel_speed_needed(*stacks.end()));
            secrets.push(Secret::between(slow * (1.0 - SECRET_MARGIN), fast * (1.0 + SECRET_MARGIN)).before(SPEED_WORDS));
        }
        if instance.kind == PuzzleKind::Ridge {
            // Many angle and speed pairs work here, so guard the whole working window.
            let layout = &instance.layout;
            let (min, max) = layout.ridge_angle_window();
            let speed_at = |angle: f64| physics_core::ballistics::level_speed(layout.pad_centre, angle, layout.g);
            secrets.push(Secret::between(min.to_degrees(), max.to_degrees()).before(ANGLE_WORDS));
            secrets.push(Secret::between(speed_at(min), speed_at(max)).before(SPEED_WORDS));
        }

        let mut known_numbers: Vec<f64> = save.notebook.entries.iter().map(|e| e.quantity.value).collect();
        // What the player committed in this puzzle's tests, in SI and in the unit it was shown in,
        // so Lumen can repeat a near miss back to them.
        for value in progress.attempts.iter().flat_map(|a| &a.values) {
            known_numbers.push(value.quantity.value);
            if let Some(unit) = physics_core::units::lookup_unit(&value.unit)
                && unit.value != 0.0
            {
                known_numbers.push(value.quantity.value / unit.value);
            }
        }
        if instance.kind == PuzzleKind::Ridge {
            known_numbers.push(instance.layout.v_max);
        }
        if instance.kind == PuzzleKind::Chase {
            // The haul cart's rating, printed on the station.
            known_numbers.push(physics_core::puzzle::Ch1Layout::CART_TOP_SPEED);
        }
        if instance.kind == PuzzleKind::Gravity
            && let Some(&notch) = save.placed.get(&def.id)
            && notch > 0
        {
            // The light gate's notch: the player chose it and it is shown when it is clamped.
            known_numbers.push(notch as f64);
        }
        if instance.kind == PuzzleKind::Channel {
            // On the launcher's rating plate, in plain view.
            known_numbers.extend([instance.layout.channel_speed_min, instance.layout.channel_speed_max]);
        }
        // The experiment's question and choices are on the player's screen (the ridge's names its
        // two angles, which may fall inside the working window).
        if let Some(demo) = &def.demo {
            for text in std::iter::once(&demo.question).chain(&demo.choices) {
                known_numbers.extend(crate::filter::number_values(text));
            }
        }

        TutorContext {
            tier_goal: String::new(),
            chapter_title: chapter.title.clone(),
            chapter_goal: chapter.goal.clone(),
            why: def.why.clone(),
            puzzle_title: def.title.clone(),
            brief: def.brief.clone(),
            how_to_measure: def.investigate.clone(),
            concepts: def
                .concepts
                .iter()
                .filter_map(|id| chapter.concept(id))
                .map(|c| ConceptCard { title: c.title.clone(), card: c.card.clone(), found: save.is_found(chapter, &c.id), hint: c.hint.clone() })
                .collect(),
            experiment: def.demo.as_ref().map(|demo| ExperimentLine {
                question: demo.question.clone(),
                choices: demo.choices.clone(),
                answer: demo.reveal.clone(),
                watched: save.demo_watched(&def.id),
                guess: save.first_guess(&def.id).and_then(|i| demo.choices.get(i).cloned()),
            }),
            placed: placed.clone(),
            asked: instance
                .inputs()
                .iter()
                .enumerate()
                .map(|(i, spec)| AskedValue {
                    label: spec.label.to_string(),
                    dimension: if spec.unit == "deg" { "an angle (enter with 'deg')".to_string() } else { spec.dim.describe() },
                    draft: drafts.get(i).cloned().unwrap_or_default(),
                    built: (spec.source == InputSource::Placed).then(|| placed.clone().unwrap_or_default()),
                })
                .collect(),
            notebook: save
                .notebook
                .entries
                .iter()
                .map(|e| NotebookLine { name: e.name.clone(), label: e.label.clone(), value: e.quantity.display(), source: e.source.clone() })
                .collect(),
            attempts: progress
                .attempts
                .iter()
                .map(|a| AttemptLine {
                    committed: a.values.iter().map(|v| format!("{} = {}", v.label, v.quantity.display_in(&v.unit))).collect(),
                    pass: a.pass,
                    headline: a.headline.clone(),
                    detail: a.detail.clone(),
                })
                .collect(),
            solved: progress.solved,
            charges_left: save.charges_left(chapter, &def.id),
            solution_steps: solution.steps.clone(),
            solution_answers: solution.answers.iter().map(|a| format!("{} = {}", a.label, a.quantity.display_in(&a.unit))).collect(),
            solution_note: solution.note.clone(),
            true_measurements: instance.givens().iter().map(|g| format!("{} = {}", g.label, g.quantity.display())).collect(),
            landmarks: Vec::new(),
            secrets,
            known_numbers,
        }
    }
}
