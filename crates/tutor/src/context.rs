//! The snapshot handed to the tutor: a plain copy of what it is allowed to know.
//!
//! This is the only thing that crosses from the game to the tutor. It is data; it holds no handle to
//! anything in the game.

use physics_core::content::{Chapter, PuzzleDef};
use physics_core::progress::Save;
use physics_core::puzzle::{Instance, PuzzleKind};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConceptCard {
    pub title: String,
    pub card: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AskedValue {
    pub label: String,
    /// "a speed (m/s)"
    pub dimension: String,
    /// What the player has typed in the blueprint so far, if anything.
    pub draft: String,
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
    /// If non-empty, a number only counts when one of these follows it ("deg", "m/s"). Used for wide
    /// ranges, where redacting every number inside would catch innocent ones.
    pub only_before: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TutorContext {
    pub chapter_title: String,
    pub puzzle_title: String,
    pub brief: String,
    pub how_to_measure: String,
    pub concepts: Vec<ConceptCard>,
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

        let mut secrets = Vec::new();
        for answer in &solution.answers {
            let mut forms = vec![answer.quantity.value];
            if let Some(unit) = physics_core::units::lookup_unit(&answer.unit)
                && unit.value != 1.0
            {
                forms.push(answer.quantity.value / unit.value);
            }
            for v in forms {
                let (a, b) = (v * (1.0 - SECRET_MARGIN), v * (1.0 + SECRET_MARGIN));
                secrets.push(Secret { low: a.min(b), high: a.max(b), only_before: Vec::new() });
            }
        }
        if instance.kind == PuzzleKind::Ridge {
            // Many angle and speed pairs work here, so guard the whole working window.
            let layout = &instance.layout;
            let (min, max) = layout.ridge_angle_window();
            let speed_at = |angle: f64| (layout.g * layout.pad_centre / (2.0 * angle).sin()).sqrt();
            secrets.push(Secret {
                low: min.to_degrees(),
                high: max.to_degrees(),
                only_before: ["deg", "°", "º"].map(String::from).to_vec(),
            });
            secrets.push(Secret {
                low: speed_at(min),
                high: speed_at(max),
                only_before: ["m/s", "m s", "metres per second", "meters per second"].map(String::from).to_vec(),
            });
        }

        let mut known_numbers: Vec<f64> = save.notebook.entries.iter().map(|e| e.quantity.value).collect();
        if instance.kind == PuzzleKind::Ridge {
            known_numbers.push(instance.layout.v_max);
        }

        TutorContext {
            chapter_title: chapter.title.clone(),
            puzzle_title: def.title.clone(),
            brief: def.brief.clone(),
            how_to_measure: def.investigate.clone(),
            concepts: def
                .concepts
                .iter()
                .filter_map(|id| chapter.concept(id))
                .map(|c| ConceptCard { title: c.title.clone(), card: c.card.clone() })
                .collect(),
            asked: instance
                .inputs()
                .iter()
                .enumerate()
                .map(|(i, spec)| AskedValue {
                    label: spec.label.to_string(),
                    dimension: if spec.unit == "deg" {
                        "an angle (enter with 'deg')".to_string()
                    } else {
                        spec.dim.describe()
                    },
                    draft: drafts.get(i).cloned().unwrap_or_default(),
                })
                .collect(),
            notebook: save
                .notebook
                .entries
                .iter()
                .map(|e| NotebookLine {
                    name: e.name.clone(),
                    label: e.label.clone(),
                    value: e.quantity.display(),
                    source: e.source.clone(),
                })
                .collect(),
            attempts: progress
                .attempts
                .iter()
                .map(|a| AttemptLine {
                    committed: a
                        .values
                        .iter()
                        .map(|v| format!("{} = {}", v.label, v.quantity.display_in(&v.unit)))
                        .collect(),
                    pass: a.pass,
                    headline: a.headline.clone(),
                    detail: a.detail.clone(),
                })
                .collect(),
            solved: progress.solved,
            charges_left: save.charges_left(chapter, &def.id),
            solution_steps: solution.steps.clone(),
            solution_answers: solution
                .answers
                .iter()
                .map(|a| format!("{} = {}", a.label, a.quantity.display_in(&a.unit)))
                .collect(),
            solution_note: solution.note.clone(),
            true_measurements: instance
                .givens()
                .iter()
                .map(|g| format!("{} = {}", g.label, g.quantity.display()))
                .collect(),
            landmarks: Vec::new(),
            secrets,
            known_numbers,
        }
    }
}
