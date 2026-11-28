//! Lumen's on-screen body: a holographic eye at the lower left that looks at things in the world,
//! points at them with a beam of light, and says short lines. While it has something to say it comes
//! forward to the bottom centre of the screen, its line above it, and goes back to its corner after.
//! Its conversation panel grows out of the eye, where its lines appear, so the eye and the panel are
//! one thing on screen. While a wake plays, the eye rises above the letterbox.
//!
//! What it looks at comes from the state of the game (the director below), or for a while from a
//! `[look: NAME]` tag in a tutor reply. Either way it is only a gaze: nothing here changes the game.
//! It points only where it helps: at the next station for a while when one opens, at places named
//! in the story, along the waking light, and to teach carrying and the ruler at the first station.
//! At later stations, during experiments and tests, and on solved puzzles the beam stays off.
//! The scripted lines never contain a number, so they cannot give an answer away.
//!
//! Lumen also carries the story: an opening in a fresh game, why each station matters when it first
//! opens, what a pass got us, and remarks about places, each said once and only when nothing else is
//! going on (see [`quiet`]). The words are in the chapter's file; this module decides when.

use crate::carry::{Carry, Item, Slot, Yard};
use crate::island::Sites;
use crate::ruler::Ruler;
use crate::sim::{Request, Sim};
use crate::state::{BuildWorld, Game, Mode, PUZZLES, Ui, not_paused};
use crate::tutor_bridge::Lumen;
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use egui::{Align2, Color32, Pos2, RichText, Stroke, Vec2 as EVec2, pos2, vec2};
use physics_core::content::{DemoDef, Dialogue, Found, RemarkWhen, Spoken, Topic};
use physics_core::puzzle::PuzzleKind;
use std::collections::BTreeSet;

/// How long Lumen keeps looking where the tutor pointed, in seconds.
const MODEL_LOOK_SECONDS: f32 = 9.0;
/// How long Lumen points the way to the next station once it opens (and at the start).
const BECKON_SECONDS: f32 = 12.0;
/// How long Lumen keeps quiet after saying anything before it makes a remark of its own, in seconds.
const REMARK_AFTER: f32 = 40.0;
/// How the camera stands while the story shows a place past the avatar: low, so the far shore and
/// the sky over it are in the picture.
const SHOW_PITCH: f32 = 0.12;
const SHOW_DISTANCE: f32 = 9.0;

/// Places Lumen can look at, by name, with where they are.
#[derive(Resource, Clone, Debug)]
pub struct Landmarks(Vec<(String, Vec3)>);

impl Landmarks {
    pub fn new(sites: &Sites, game: &Game) -> Self {
        // Pins on things that come and go (the light gate) are places to look at only while they are
        // in the world; `follow` keeps them in step.
        let mut places: Vec<(String, Vec3)> = sites.pins.iter().filter(|pin| pin.present()).map(|pin| (pin.label.to_string(), pin.position)).collect();
        for (i, console) in sites.stations.iter().enumerate() {
            // The station's screen, under its column of light.
            places.push((station_name(game, i), console.as_vec3() + Vec3::new(0.5, 1.3, 0.5)));
        }
        let pile = sites.block_pile.iter().map(|c| c.as_vec3()).sum::<Vec3>() / sites.block_pile.len().max(1) as f32;
        places.push((LEDGE_BLOCKS.into(), pile + Vec3::new(0.5, 1.5, 0.5)));
        places.push((JACK_FRAME.into(), sites.channel_well.as_vec3() + Vec3::new(0.5, 0.5, 0.5)));
        places.push((CART_BLOCK.into(), sites.cargo_cell.as_vec3() + Vec3::new(0.5, 1.0, 0.5)));
        places.push((RAIL_CART.into(), sites.cart + Vec3::Y * 0.8));
        places.push((LIGHT_GATE.into(), sites.gate_stand + Vec3::Y * 1.2));
        places.push((TOWER_NOTCHES.into(), (sites.tower_release + sites.tower_plate) / 2.0 - Vec3::X * 1.5));
        places.push(("The wreck".into(), sites.wreck + Vec3::Y * 2.0));
        places.push(("The boat on the slipway".into(), sites.slipway + Vec3::Y * 1.0));
        places.push((MAIN_HULL.into(), crate::intro::Mainland::wreck() + Vec3::Y * 6.0));
        places.push((SHORE.into(), sites.shore + Vec3::Y * 1.0));
        // The way: its gates and the ramp, and the light that runs along it while a machine wakes.
        let way = &sites.way;
        places.push(("The stone gate".into(), way.stone_gate + Vec3::Y * 2.0));
        places.push(("The north gate".into(), way.keystone));
        places.push(("The ramp".into(), way.ramp.centre() + Vec3::Y * 0.5));
        places.push(("The bridge".into(), way.bridge.centre() + Vec3::Y * 1.0));
        places.push(("The seal".into(), way.seal.centre() + Vec3::Y * 0.5));
        places.push((crate::waking::LIGHT_ON_THE_ROAD.into(), way.machines[0]));
        places.push((BEAM.into(), sites.beam.pivot + Vec3::Y * 1.0));
        Landmarks(places)
    }

    /// Follow what moves with what is placed: the survey pins (the ledge launcher's release point
    /// rides up the jack frame, the light gate's comes and goes with the gate) and the rail cart,
    /// which stays where its last run stopped.
    pub fn follow(&mut self, sites: &Sites) {
        for pin in &sites.pins {
            let at = self.0.iter().position(|(name, _)| name == pin.label);
            match (at, pin.present()) {
                (Some(i), true) => self.0[i].1 = pin.position,
                (Some(i), false) => {
                    self.0.remove(i);
                }
                (None, true) => self.0.push((pin.label.to_string(), pin.position)),
                (None, false) => {}
            }
        }
        self.set(RAIL_CART, sites.cart + Vec3::Y * 0.8);
    }

    /// Move a place that moves (the light running up the road).
    pub fn set(&mut self, name: &str, at: Vec3) {
        if let Some(place) = self.0.iter_mut().find(|(n, _)| n == name) {
            place.1 = at;
        }
    }

    pub fn names(&self) -> Vec<String> {
        self.0.iter().map(|(name, _)| name.clone()).collect()
    }

    pub fn position(&self, name: &str) -> Option<Vec3> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, p)| *p)
    }
}

/// What Lumen says the first time deep water stops the engineer.
const WATER_LINE: &str =
    "Oh, thank goodness, you scrambled back to the bank. It gets deep fast out there, and I couldn't have pulled you out. Let's keep to the shallows.";

/// The blocks on the ledge, and the frame at its edge they go into.
const LEDGE_BLOCKS: &str = "Blocks on the ledge";
const JACK_FRAME: &str = "Jack frame under the ledge launcher";
/// The block for the supply rail's cart where it lies at the start, and the cart it goes on.
const CART_BLOCK: &str = "The cart's block beside the rail";
const RAIL_CART: &str = "Rail cart";
/// The light gate on its stand, and the drop tower it clamps on.
const LIGHT_GATE: &str = "Light gate on its stand";
const TOWER_NOTCHES: &str = "Notches on the drop tower";
/// The beam, the side-machine off the road.
const BEAM: &str = "The beam";
/// The main hull on the mainland, where the rest of Lumen is, and the island's shore facing it.
const MAIN_HULL: &str = "The main hull, across the strait";
const SHORE: &str = "The shore";

pub fn station_name(game: &Game, puzzle: usize) -> String {
    format!("Station: {}", game.def(puzzle).title)
}

/// The two survey pins Lumen guides the ruler between at the first station (the supply rail).
const RAIL_MEASURE: [&str; 2] = ["Rail cart: starting point", "Rail marker: centre"];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mood {
    #[default]
    Neutral,
    Pleased,
    Concerned,
    Thinking,
}

/// Which console currently owns the active interactive dialogue.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Console {
    Station(usize),
    Beam,
}

/// Hands-on steps of Station 0's action-gated ruler and notebook walkthrough.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GuideStep {
    /// Waiting for the player to press R to bring out the ruler.
    PressR,
    /// Ruler is active; waiting for the first yellow pin click.
    PickFirstPin,
    /// First pin clicked; waiting for the second yellow pin click.
    PickSecondPin,
    /// Finished reading is on screen; waiting for the player to note a value in the notebook.
    NoteReading,
    /// Reading saved; waiting for the player to open the notebook (N or clicking the header).
    OpenNotebook,
}

impl GuideStep {
    fn text(self) -> &'static str {
        match self {
            GuideStep::PressR => "Before we open the console, let's measure to the orange marker together. Press R to bring out the ruler.",
            GuideStep::PickFirstPin => "Click the yellow pin I'm pointing at.",
            GuideStep::PickSecondPin => "And now this one.",
            GuideStep::NoteReading => "Now click Note on the ruler card to save the horizontal distance to your notebook.",
            GuideStep::OpenNotebook => "Saved! Press N to open your notebook and see where your readings and calculator live.",
        }
    }

    fn cue(self) -> &'static str {
        match self {
            GuideStep::PressR => "Press R to bring out the ruler",
            GuideStep::PickFirstPin => "Click the first yellow pin",
            GuideStep::PickSecondPin => "Click the second yellow pin",
            GuideStep::NoteReading => "Click Note on the ruler card",
            GuideStep::OpenNotebook => "Press N or click Notebook",
        }
    }

    fn look(self) -> Option<&'static str> {
        match self {
            GuideStep::PressR => Some("Rail marker: centre"),
            _ => None,
        }
    }
}

/// Lumen's line under a reveal that wrote the player's first handbook pages, until they open it.
const FIRST_PAGES_LINE: &str = "That's gone into your handbook. Press H to open it and have a look.";

/// Spoken line Lumen says once the Station 0 ruler and notebook walkthrough is complete.
const GUIDE_DONE_LINE: &str = "There it is. Your notebook keeps every reading with its unit, and the calculator underneath can use their names. Whenever you're ready, open the console and let's get that cart loaded.";

/// What follows the final presentation beat of a station's initial briefing.
#[derive(Clone, Debug)]
enum NextBeat {
    /// Station 0's action-gated ruler and notebook walkthrough.
    MeasureGuide,
    /// A station's conceptual "Try it first" prediction prompt.
    Experiment(usize, DemoDef),
}

/// What phase Lumen's interactive station dialogue is currently showing.
#[derive(Clone, Debug)]
enum DialogueStage {
    /// Stepping through presentation beats or a topic's reply lines.
    Lines(Vec<Spoken>),
    /// Action-gated hands-on ruler and notebook walkthrough at Station 0.
    Guide(GuideStep),
    /// Asking the station's "Try it first" conceptual prediction question before running it.
    Experiment { puzzle: usize, demo: DemoDef },
    /// After the experiment finishes: Lumen's verdict, full reveal, and newly unlocked handbook pages.
    Reveal {
        puzzle: usize,
        /// Optional verdict line ("You called it!" / "Not what you expected?...") and whether it matched.
        verdict: Option<(String, bool)>,
        /// Full `demo.reveal` explanation in Lumen's voice.
        reveal: String,
        /// Newly written handbook pages `(concept_id, title)` from this experiment.
        new_pages: Vec<(String, String)>,
    },
}

/// Interactive station dialogue shown in Lumen's bubble: presentation beats, optional action-gated
/// walkthrough or "Try it first" experiment prompt and reveal, and clickable Q&A chips.
#[derive(Clone, Debug)]
struct StationDialogue {
    console: Console,
    topics: Vec<Topic>,
    /// What follows the final presentation beat, if anything.
    pending: Option<NextBeat>,
    /// Keep the right-hand station / beam console panel hidden while this initial briefing plays.
    hide_panel: bool,
    stage: DialogueStage,
    shown: f32,
}

impl StationDialogue {
    /// Whether Enter / Next advances to another beat (or into the pending guide / experiment).
    fn has_more(&self) -> bool {
        match &self.stage {
            DialogueStage::Lines(lines) => lines.len() > 1 || self.pending.is_some(),
            DialogueStage::Guide(_) | DialogueStage::Experiment { .. } | DialogueStage::Reveal { .. } => false,
        }
    }

    /// The landmark Lumen is pointing out on the current beat, if any.
    fn look(&self) -> Option<&str> {
        match &self.stage {
            DialogueStage::Lines(lines) => lines.first().and_then(|l| l.look.as_deref()),
            DialogueStage::Guide(step) => step.look(),
            DialogueStage::Experiment { .. } | DialogueStage::Reveal { .. } => None,
        }
    }
}

/// Action-gated prompt shown the first time a handbook page is unlocked, waiting until the player
/// opens the handbook (`H` or clicking its header) at least once.
#[derive(Clone, Debug)]
struct HandbookPrompt {
    text: String,
    shown: f32,
}

/// Tracks a conceptual experiment launched from Lumen's dialogue so the post-experiment reveal
/// resumes cleanly when the simulation finishes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AwaitingDemo {
    puzzle: usize,
    hide_panel: bool,
    /// True on the first predicted run (show verdict + reveal); false on a replay.
    first_run: bool,
    /// Whether the station has started the run. Until it has, a request it dropped would leave
    /// this waiting (and the console hidden) for good, so it is forgotten instead.
    started: bool,
}

#[derive(Clone, Debug)]
struct Line {
    text: String,
    left: f32,
    shown: f32,
    /// A place the eye holds while the line is said.
    look: Option<String>,
    /// Part of a story (the opening or closing): what is said meanwhile waits its turn instead of cutting it.
    keep: bool,
    /// Waits for the player to click or press Enter instead of fading out automatically.
    wait: bool,
    /// The frame it was said in.
    said: u64,
}

/// How many recent spoken lines Lumen keeps in its log for the player to reread.
const MAX_HISTORY: usize = 40;

/// Lumen's state: where it looks, how it feels, what it is saying, and its animation.
#[derive(Resource, Default)]
pub struct Companion {
    /// Name of the place Lumen is looking at, if any (one of the [`Landmarks`]).
    pub gaze: Option<String>,
    pub mood: Mood,
    /// A gaze from the tutor (`[look: NAME]`) that outranks the scripted one for a while.
    held: Option<(String, f32)>,
    mood_left: f32,
    lines: Vec<Line>,
    /// Active interactive briefing/Q&A dialogue at an open console, if any.
    dialogue: Option<StationDialogue>,
    /// Experiment launched from Lumen's dialogue whose post-run reveal should open when it ends.
    awaiting_demo: Option<AwaitingDemo>,
    /// Action-gated prompt waiting for the player to open the handbook the first time a page arrives.
    handbook_prompt: Option<HandbookPrompt>,
    /// Whether the player has already opened the handbook (or loaded a save with pages already found).
    handbook_opened: bool,
    /// Topics the player has already asked this session, so their chips show a subtle tick.
    asked: BTreeSet<(Console, usize)>,
    /// Consoles whose automatic presentation has already played this session.
    presented: BTreeSet<Console>,
    /// Recent lines Lumen has spoken this session, oldest first, shown in Lumen's panel.
    pub history: Vec<String>,
    anim: Animation,
    /// Stations where Lumen has already given a one-time post-miss/recharge nudge this session.
    nudged: BTreeSet<(usize, Nudge)>,
    /// Seconds left of pointing the way to the next station.
    beckon_left: f32,
    /// Seconds since Lumen last said anything.
    since_spoke: f32,
    /// Remarks already made this session, by their index in the chapter.
    remarked: BTreeSet<usize>,
    /// Frames directed so far, to tell lines said in the same moment.
    frame: u64,
    /// The place the camera has turned to show while the story names it.
    showing: Option<String>,
    /// Whether Lumen has said why the engineer stopped at deep water, this session.
    warned_of_water: bool,
    /// At the beam, this session: the releases that missed since a load last held, and whether
    /// Lumen has suggested looking for a rule.
    beam_misses: u32,
    beam_rule: bool,
    /// Last frame's view of the game, stored here so world reloads reset it cleanly.
    last_seen: Option<Seen>,
}

impl Companion {
    /// Whether a story (the opening or closing) is still being told.
    pub fn telling(&self) -> bool {
        self.lines.iter().any(|l| l.keep)
    }

    /// Whether the current bubble is waiting for the player to advance it.
    #[cfg(test)]
    pub fn waiting_line(&self) -> bool {
        self.dialogue.is_some() || self.lines.first().is_some_and(|l| l.wait)
    }

    /// Whether the bubble is waiting and has been up as long as a timed line of its length would
    /// be, so a script can advance it at a reading pace.
    pub fn waiting_line_read(&self) -> bool {
        self.dialogue.is_some() || self.lines.first().is_some_and(|l| l.wait && l.shown >= l.left)
    }

    /// Seconds until the timed lines waiting to be said have been said.
    fn talking_for(&self) -> f32 {
        self.lines.iter().filter(|l| !l.wait).map(|l| l.left).sum()
    }

    fn say(&mut self, text: impl Into<String>) {
        self.dialogue = None;
        self.speak(text.into(), None, false, false);
    }

    /// Say one of the chapter's lines, holding the eye on its place, if it names one.
    /// Story lines (`keep == true`, opening and closing) wait for click / Enter.
    fn say_spoken(&mut self, line: &Spoken, keep: bool) {
        self.speak(line.text.clone(), line.look.clone(), keep, keep);
    }

    /// Whether the right-hand station or beam console panel should stay hidden while Lumen
    /// finishes a story beat, delivers its initial briefing, or waits for the first experiment to
    /// finish.
    pub fn hiding_console_panel(&self) -> bool {
        self.telling() || self.dialogue.as_ref().is_some_and(|d| d.hide_panel) || self.awaiting_demo.as_ref().is_some_and(|a| a.hide_panel)
    }

    /// Open Lumen's interactive station dialogue. On first open (`full_present == true`) every
    /// presentation beat plays in sequence (and leads into `pending` if present); on manual
    /// Briefing (`full_present == false`) it opens directly on the final beat with all topic chips
    /// immediately clickable.
    fn open_dialogue(&mut self, console: Console, dialogue: &Dialogue, full_present: bool, hide_panel: bool, pending: Option<NextBeat>) {
        let lines: Vec<Spoken> = if full_present { dialogue.present.clone() } else { dialogue.present.last().cloned().into_iter().collect() };
        if lines.is_empty() {
            return;
        }
        self.hand_over_handbook_gate();
        // A story still being told is kept: it carries on once the dialogue closes.
        self.lines.retain(|l| l.keep);
        self.record_history(&lines[0].text);
        self.dialogue = Some(StationDialogue {
            console,
            topics: dialogue.topics.clone(),
            pending: if full_present { pending } else { None },
            hide_panel,
            stage: DialogueStage::Lines(lines),
            shown: 0.0,
        });
        self.since_spoke = 0.0;
    }

    /// Open Lumen's "Try it first" prediction prompt directly for `puzzle`.
    fn open_experiment(&mut self, puzzle: usize, dialogue: &Dialogue, demo: DemoDef, hide_panel: bool) {
        self.hand_over_handbook_gate();
        self.lines.retain(|l| l.keep);
        self.dialogue = Some(StationDialogue {
            console: Console::Station(puzzle),
            topics: dialogue.topics.clone(),
            pending: None,
            hide_panel,
            stage: DialogueStage::Experiment { puzzle, demo },
            shown: 0.0,
        });
        self.since_spoke = 0.0;
    }

    /// Open Lumen's post-experiment reveal bubble with verdict, full explanation, new handbook
    /// pages, and follow-up station topics.
    fn open_demo_reveal(
        &mut self,
        puzzle: usize,
        dialogue: &Dialogue,
        verdict: Option<(String, bool)>,
        reveal: String,
        new_pages: Vec<(String, String)>,
        hide_panel: bool,
    ) {
        self.lines.retain(|l| l.keep);
        if let Some((line, _)) = &verdict {
            self.record_history(line);
        }
        self.record_history(&reveal);
        self.dialogue = Some(StationDialogue {
            console: Console::Station(puzzle),
            topics: dialogue.topics.clone(),
            pending: None,
            hide_panel,
            stage: DialogueStage::Reveal { puzzle, verdict, reveal, new_pages },
            shown: 0.0,
        });
        self.since_spoke = 0.0;
    }

    /// Advance the active station dialogue to its next beat (or into its pending guide / experiment),
    /// or dismiss it if on the last beat.
    fn advance_dialogue(&mut self) -> bool {
        let Some(dialogue) = &mut self.dialogue else { return false };
        match &mut dialogue.stage {
            DialogueStage::Lines(lines) if lines.len() > 1 => {
                lines.remove(0);
                dialogue.shown = 0.0;
                let text = lines[0].text.clone();
                self.record_history(&text);
                self.since_spoke = 0.0;
            }
            DialogueStage::Lines(_) if dialogue.pending.is_some() => {
                let next = dialogue.pending.take().expect("checked above");
                dialogue.shown = 0.0;
                self.since_spoke = 0.0;
                match next {
                    NextBeat::MeasureGuide => {
                        dialogue.stage = DialogueStage::Guide(GuideStep::PressR);
                        self.record_history(GuideStep::PressR.text());
                    }
                    NextBeat::Experiment(puzzle, demo) => {
                        dialogue.stage = DialogueStage::Experiment { puzzle, demo };
                    }
                }
            }
            _ => {
                self.dialogue = None;
            }
        }
        true
    }

    /// Drive Station 0's action-gated ruler and notebook walkthrough forward (or recover if the
    /// ruler is cancelled early) based on live ruler, notebook, and UI state.
    fn sync_measure_guide(&mut self, ruler: &Ruler, has_reading: bool, notebook_open: bool) {
        let Some(dialogue) = &mut self.dialogue else { return };
        let DialogueStage::Guide(current) = dialogue.stage else { return };
        let next = if has_reading {
            if notebook_open { None } else { Some(GuideStep::OpenNotebook) }
        } else if ruler.finished() {
            Some(GuideStep::NoteReading)
        } else if ruler.active && ruler.first.is_some() {
            Some(GuideStep::PickSecondPin)
        } else if ruler.active {
            Some(GuideStep::PickFirstPin)
        } else {
            Some(GuideStep::PressR)
        };
        match next {
            Some(step) if step != current => {
                dialogue.stage = DialogueStage::Guide(step);
                dialogue.shown = 0.0;
                self.record_history(step.text());
                self.since_spoke = 0.0;
            }
            None => {
                dialogue.stage = DialogueStage::Lines(vec![Spoken { text: GUIDE_DONE_LINE.into(), look: Some(CART_BLOCK.into()) }]);
                dialogue.shown = 0.0;
                self.record_history(GUIDE_DONE_LINE);
                self.since_spoke = 0.0;
                self.feel(Mood::Pleased, 3.0);
            }
            _ => {}
        }
    }

    /// Whether the open reveal has just written the player's first handbook pages. Until they open
    /// the handbook, the reveal offers only those pages: its topic chips, `Run it again` and
    /// `Open console` wait.
    fn reveal_waits_for_handbook(&self) -> bool {
        !self.handbook_opened && self.dialogue.as_ref().is_some_and(|d| matches!(&d.stage, DialogueStage::Reveal { new_pages, .. } if !new_pages.is_empty()))
    }

    /// The reveal is closing (or being replaced) before the player has opened the handbook: the
    /// first-time handbook prompt takes over, so the tutorial step isn't skipped.
    fn hand_over_handbook_gate(&mut self) {
        if !self.reveal_waits_for_handbook() || self.handbook_prompt.is_some() {
            return;
        }
        let title = match self.dialogue.as_ref().map(|d| &d.stage) {
            Some(DialogueStage::Reveal { new_pages, .. }) => new_pages.first().map(|(_, title)| title.clone()),
            _ => None,
        };
        let Some(title) = title else { return };
        let text = first_page_prompt(&title);
        self.record_history(&text);
        self.handbook_prompt = Some(HandbookPrompt { text, shown: 0.0 });
    }

    /// Follow an experiment Lumen is waiting on, given whether a request is still queued for the
    /// station and which experiment is running. It marks the run started once it plays, and forgets
    /// it once it has ended (its reveal opened already) or if the station dropped the request
    /// without running it, so the console isn't left hidden.
    fn track_awaiting_demo(&mut self, request_queued: bool, running_demo: Option<usize>) {
        let Some(awaiting) = &mut self.awaiting_demo else { return };
        if running_demo == Some(awaiting.puzzle) {
            awaiting.started = true;
        } else if awaiting.started || !request_queued {
            self.awaiting_demo = None;
        }
    }

    /// Answer a clicked station dialogue topic chip.
    fn pick_topic(&mut self, index: usize) -> bool {
        let Some(dialogue) = &mut self.dialogue else { return false };
        let Some(topic) = dialogue.topics.get(index).cloned() else { return false };
        if topic.lines.is_empty() {
            return false;
        }
        let console = dialogue.console;
        let first = topic.lines[0].text.clone();
        dialogue.stage = DialogueStage::Lines(topic.lines);
        dialogue.shown = 0.0;
        self.asked.insert((console, index));
        self.record_history(&first);
        self.since_spoke = 0.0;
        true
    }

    /// Skip straight to the next queued line, or dismiss the bubble if this is the last one.
    pub fn advance_line(&mut self) -> bool {
        if self.dialogue.is_some() {
            return self.advance_dialogue();
        }
        if self.lines.is_empty() {
            return false;
        }
        self.lines.remove(0);
        true
    }

    fn record_history(&mut self, text: &str) {
        if self.history.last().map(String::as_str) == Some(text) {
            return;
        }
        self.history.push(text.to_string());
        if self.history.len() > MAX_HISTORY {
            let drop = self.history.len() - MAX_HISTORY;
            self.history.drain(..drop);
        }
    }

    fn speak(&mut self, text: String, look: Option<String>, keep: bool, wait: bool) {
        self.record_history(&text);
        let telling = self.lines.iter().any(|l| l.keep);
        if telling {
            let now = self.frame;
            self.lines.retain(|l| l.keep || l.shown > 0.0 || l.said == now);
        } else if let Some(current) = self.lines.first_mut()
            && current.shown > 0.0
        {
            current.left = current.left.min(0.3);
            self.lines.truncate(1);
        }
        let words = text.split_whitespace().count() as f32;
        // Timed lines stay long enough to read, then fade out; waiting lines stay until advanced.
        self.lines.push(Line { text, left: (2.5 + words * 0.32).min(10.0), shown: 0.0, look, keep, wait, said: self.frame });
        self.since_spoke = 0.0;
    }

    /// Make a remark, if Lumen is not saying anything else.
    fn remark(&mut self, line: &Spoken) -> bool {
        if !self.lines.is_empty() || self.dialogue.is_some() {
            return false;
        }
        self.say_spoken(line, false);
        true
    }

    fn feel(&mut self, mood: Mood, seconds: f32) {
        self.mood = mood;
        self.mood_left = seconds;
    }

    fn hold(&mut self, place: &str, seconds: f32) {
        self.held = Some((place.to_string(), seconds));
    }
}

/// What Lumen says once per station per session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Nudge {
    /// After a miss with the player's own g: it held up at the drop tower.
    YourG,
    /// The station has run out of charges.
    Recharge,
    /// After two misses, if the player has not talked to Lumen about the puzzle.
    AskMe,
}

/// What decides whether Lumen may make a remark of its own. A plain copy, so it can be tested.
#[derive(Clone, Copy, Debug, Default)]
struct Quiet {
    station_open: bool,
    /// A test or an experiment is running.
    running: bool,
    ruler: bool,
    conversation_open: bool,
    carrying: bool,
    /// Something is being said, or waits to be.
    speaking: bool,
    since_spoke: f32,
}

/// Lumen remarks on a place only when the player is not busy and it has been quiet for a while: a
/// remark never gets in the way of the physics.
fn quiet(q: &Quiet) -> bool {
    !(q.station_open || q.running || q.ruler || q.conversation_open || q.carrying || q.speaking) && q.since_spoke >= REMARK_AFTER
}

/// Whether a remark's condition holds.
fn remark_due(when: &RemarkWhen, game: &Game) -> bool {
    // A side-machine counts once it has woken.
    let solved = |id: &str| game.chapter.puzzles.iter().position(|p| p.id == id).is_some_and(|i| game.solved(i)) || game.save.side_woken(id);
    match when {
        RemarkWhen::Always => true,
        RemarkWhen::Before(id) => !solved(id),
        RemarkWhen::After(id) => solved(id),
    }
}

/// What the director looks at to decide where Lumen's eye goes. A plain copy, so the decision can
/// be tested without the engine.
#[derive(Clone, Debug, Default)]
struct Facts {
    /// Kind of the puzzle the player is working on (`None` at a side-machine).
    focus_kind: Option<PuzzleKind>,
    /// A station or side-machine console is open.
    station_open: bool,
    focus_solved: bool,
    /// The next puzzle not yet solved, if any.
    next: Option<usize>,
    next_station: Option<String>,
    /// A test, experiment, measurement or beam release is running.
    running: bool,
    /// The ruler is taking points.
    ruler_active: bool,
    /// Label of the ruler's first point, while it waits for the second.
    ruler_first: Option<String>,
    /// Lumen is pointing the way to the next station (for a while after it opens).
    beckon: bool,
    /// The avatar stands at a station that is solved or still locked: the way on is elsewhere.
    at_finished_station: bool,
    /// A place the tutor asked Lumen to look at.
    held: Option<String>,
    /// The place the active station dialogue line is pointing out.
    dialogue_look: Option<String>,
    /// The place the line being said is about: the eye holds it when nothing is going on.
    line_look: Option<String>,
    /// What the avatar is carrying.
    carrying: Option<Item>,
    /// The light is running up the road to wake a machine.
    light_running: bool,
    /// The rail cart has no cargo yet (first station tutorial).
    slot_empty: bool,
}

/// Where Lumen should look, in priority order.
fn choose_gaze(f: &Facts) -> Option<String> {
    if f.light_running {
        return Some(crate::waking::LIGHT_ON_THE_ROAD.into());
    }
    if f.running {
        return None;
    }
    if let Some(held) = &f.held {
        return Some(held.clone());
    }
    if let Some(look) = &f.dialogue_look {
        return Some(look.clone());
    }
    // Only the first station (the supply rail) gets step-by-step beam guidance, to teach the ruler
    // and carrying; later stations leave the screen clear so the player works out what to measure.
    let tutorial = !f.focus_solved && f.focus_kind == Some(PuzzleKind::Rail);
    if tutorial {
        if f.ruler_active {
            let [a, b] = RAIL_MEASURE;
            return Some(match &f.ruler_first {
                None => a.into(),
                // Whichever of the two has not been clicked yet.
                Some(first) => if first == b { a } else { b }.into(),
            });
        }
        if f.carrying == Some(Item::Block(Yard::Rail)) {
            return Some(RAIL_CART.into());
        }
        if f.station_open && f.slot_empty {
            return Some(CART_BLOCK.into());
        }
    }
    if f.station_open {
        return None;
    }
    if let Some(look) = &f.line_look {
        return Some(look.clone());
    }
    if !(f.beckon || f.at_finished_station) {
        return None;
    }
    match &f.next_station {
        Some(station) => Some(station.clone()),
        None if f.next.is_none() => Some("The boat on the slipway".into()),
        None => None,
    }
}

/// Last frame's view of the game, to notice what has just happened.
#[derive(Clone, Default)]
struct Seen {
    results: [bool; PUZZLES],
    refusal: Option<String>,
    next: Option<usize>,
    ruler_active: bool,
    ruler_first: bool,
    asking: bool,
    /// Experiments watched at least once, per puzzle.
    watched: [bool; PUZZLES],
    /// Conceptual experiment currently playing out, if any.
    demo_running: Option<usize>,
    found: BTreeSet<String>,
    /// Handbook pages whose quiz has been answered in full.
    quizzes: BTreeSet<String>,
    station: Option<usize>,
    recharging: [bool; PUZZLES],
    beam_releases: u32,
    beam_woken: bool,
}

pub struct CompanionPlugin;

impl Plugin for CompanionPlugin {
    fn build(&self, app: &mut App) {
        let landmarks = Landmarks::new(app.world().resource::<Sites>(), app.world().resource::<Game>());
        app.insert_resource(landmarks)
            .init_resource::<Companion>()
            .add_systems(BuildWorld, reset)
            // The greeting comes with the first frame of the game, after Start.
            .add_systems(
                Update,
                (
                    follow_sites,
                    advance_speech.run_if(in_state(Mode::Playing)).run_if(not_paused),
                    direct.after(crate::stations::proximity).after(crate::beam::proximity).run_if(in_state(Mode::Playing)).run_if(not_paused),
                )
                    .chain(),
            )
            .add_systems(EguiPrimaryContextPass, draw.after(crate::ui::draw).after(crate::beam::panel).run_if(in_state(Mode::Playing)));
    }
}

fn reset(sites: Res<Sites>, game: Res<Game>, mut landmarks: ResMut<Landmarks>, mut companion: ResMut<Companion>) {
    *landmarks = Landmarks::new(&sites, &game);
    *companion = Companion::default();
}

/// Pressing Enter advances or dismisses Lumen's current speech bubble or station dialogue
/// whenever no text field has keyboard focus.
fn advance_speech(
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    ui: Res<Ui>,
    waking: Res<crate::waking::Waking>,
    mut companion: ResMut<Companion>,
) {
    if wants.wants_any_keyboard_input() || ui.tutor_open || waking.playing() {
        return;
    }
    if keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter]) {
        if let Some(dialogue) = &companion.dialogue {
            // Enter steps through presentation beats (and into the experiment prompt); once choices
            // or topic chips are showing, the player clicks one or dismisses explicitly.
            if dialogue.has_more() {
                companion.advance_dialogue();
            }
        } else if !ui.at_console() || companion.hiding_console_panel() {
            companion.advance_line();
        }
    }
}

/// Keep the landmarks on what moves (see [`Landmarks::follow`]).
fn follow_sites(sites: Res<Sites>, mut landmarks: ResMut<Landmarks>) {
    if sites.is_changed() {
        landmarks.follow(&sites);
    }
}

#[allow(clippy::too_many_arguments)]
fn direct(
    time: Res<Time>,
    game: Res<Game>,
    mut ui: ResMut<Ui>,
    sim: Res<Sim>,
    ruler: Res<Ruler>,
    carry: Res<Carry>,
    landmarks: Res<Landmarks>,
    waking: Res<crate::waking::Waking>,
    player: Single<&Transform, With<crate::player::Player>>,
    shore: Res<crate::player::Shore>,
    mut orbit: ResMut<crate::player::OrbitCamera>,
    mut lumen: ResMut<Lumen>,
    mut companion: ResMut<Companion>,
    beam: Res<crate::beam::Beam>,
) {
    let dt = time.delta_secs();
    let companion = &mut *companion;
    companion.frame += 1;
    let focus = ui.focus(&game);
    let focus_kind = game.def(focus).kind;
    // The next job as the world shows it: after a pass, Lumen points on once the way has woken the
    // next machine and its console is in, and closes the chapter once the north gate is lit.
    let next = (waking.shown < PUZZLES).then_some(waking.shown);

    // The tutor pointed somewhere.
    for conversation in &mut lumen.conversations {
        if let Some(place) = conversation.take_look()
            && landmarks.position(&place).is_some()
        {
            companion.hold(&place, MODEL_LOOK_SECONDS);
        }
    }
    let asking = lumen.conversations.iter().any(|c| c.is_waiting());

    let now = Seen {
        results: std::array::from_fn(|i| ui.results[i].is_some()),
        refusal: ui.refusal.clone(),
        next,
        ruler_active: ruler.active,
        ruler_first: ruler.first.is_some(),
        asking,
        watched: std::array::from_fn(|i| game.save.demo_watched(&game.def(i).id)),
        demo_running: sim.demo(),
        found: game.save.found.clone(),
        quizzes: game.chapter.concepts.iter().filter(|c| game.save.quiz_done(&game.chapter, &c.id)).map(|c| c.id.clone()).collect(),
        station: ui.station,
        recharging: std::array::from_fn(|i| ui.recharging[i].is_some()),
        beam_releases: beam.releases,
        beam_woken: crate::beam::woken(&game),
    };
    match companion.last_seen.clone() {
        None => {
            // A reloaded save that already discovered pages does not need the first-time handbook prompt.
            if !game.save.found.is_empty() {
                companion.handbook_opened = true;
            }
            // The first frame: a greeting. The opening plays until the player has done anything at
            // all, so it isn't heard again by someone who has already been measuring.
            let fresh = game.save.untouched();
            match next {
                Some(n) if fresh => {
                    // The opening is a story: it is not cut short, and Lumen points the way after it.
                    for line in &game.chapter.opening {
                        companion.say_spoken(line, true);
                    }
                    let first = Spoken {
                        text: if game.def(n).kind == PuzzleKind::Rail {
                            "There's one stone pedestal still glowing by that cart in the groove. Follow my beam, and let's see what it does.".into()
                        } else {
                            format!("{} Follow my beam, and I'll show you the station.", game.def(n).task)
                        },
                        look: None,
                    };
                    companion.say_spoken(&first, true);
                }
                Some(n) => companion.say(format!("You're back! Next we need to {}", lowercase_first(&game.def(n).task))),
                None => companion.say("You're back! The skiff has its helm, and it's still glowing."),
            }
            companion.beckon_left = BECKON_SECONDS + companion.talking_for();
        }
        Some(before) => {
            for puzzle in 0..PUZZLES {
                if now.results[puzzle] && !before.results[puzzle] {
                    let ask_me = lumen.tutor.is_online() && lumen.conversations[puzzle].turns.is_empty();
                    react_to_result(companion, &game, &ui, puzzle, ask_me);
                }
                if now.recharging[puzzle] && !before.recharging[puzzle] && companion.nudged.insert((puzzle, Nudge::Recharge)) {
                    companion.say("It's recharging. While we wait, let's check your readings against the handbook.");
                }
            }
            if shore.fallen_in > 0 && !companion.warned_of_water {
                companion.warned_of_water = true;
                companion.feel(Mood::Concerned, 4.0);
                companion.say(WATER_LINE);
            }
            if now.refusal.is_some() && now.refusal != before.refusal {
                companion.feel(Mood::Concerned, 4.0);
                companion.say("The console rejected that before anything moved, so it didn't spend a charge. The reason's under the button.");
            }
            let mut revealed_pages: BTreeSet<String> = BTreeSet::new();
            for puzzle in 0..PUZZLES {
                if now.watched[puzzle] && !before.watched[puzzle] {
                    if ui.station == Some(puzzle) && !ui.tutor_open && (!companion.telling() || companion.awaiting_demo.is_some()) {
                        let hide_panel = companion.awaiting_demo.take().filter(|a| a.puzzle == puzzle).is_some_and(|a| a.hide_panel);
                        let def = game.def(puzzle);
                        let new_pages: Vec<(String, String)> = now
                            .found
                            .difference(&before.found)
                            .filter_map(|id| game.chapter.concept(id))
                            .filter(|c| c.found == Found::Demo(def.id.clone()))
                            .map(|c| {
                                revealed_pages.insert(c.id.clone());
                                (c.id.clone(), c.title.clone())
                            })
                            .collect();
                        open_demo_reveal(companion, &game, puzzle, new_pages, hide_panel, true);
                    } else {
                        companion.awaiting_demo = None;
                        react_to_demo(companion, &game, puzzle);
                    }
                }
            }
            if let Some(puzzle) = before.demo_running
                && now.demo_running.is_none()
                && let Some(awaiting) = companion.awaiting_demo.filter(|a| a.puzzle == puzzle && !a.first_run)
            {
                companion.awaiting_demo = None;
                if ui.station == Some(puzzle) && !ui.tutor_open {
                    open_demo_reveal(companion, &game, puzzle, Vec::new(), awaiting.hide_panel, false);
                }
            }
            if now.beam_releases > before.beam_releases {
                react_to_release(companion, &game, &beam);
            }
            if now.beam_woken && !before.beam_woken {
                companion.feel(Mood::Pleased, 6.0);
                if let Some(side) = game.chapter.side(crate::beam::BEAM_ID) {
                    companion.say(side.done.clone());
                }
            }
            for id in now.found.difference(&before.found) {
                if revealed_pages.contains(id) {
                    continue;
                }
                if let Some(page) = game.chapter.concept(id) {
                    companion.feel(Mood::Pleased, 4.0);
                    if !companion.handbook_opened && companion.handbook_prompt.is_none() {
                        let text = first_page_prompt(&page.title);
                        companion.record_history(&text);
                        companion.handbook_prompt = Some(HandbookPrompt { text, shown: 0.0 });
                    } else {
                        companion.say(format!("There's a new page in your handbook, {}. Press H to read it.", page.title));
                    }
                }
            }
            for id in now.quizzes.difference(&before.quizzes) {
                if let Some(page) = game.chapter.concept(id) {
                    companion.feel(Mood::Pleased, 4.0);
                    companion.say(format!("You got every question on {} right. I'm impressed.", page.title));
                }
            }
            if now.station != before.station && now.station == next {
                companion.beckon_left = 0.0;
            }
            if now.next != before.next {
                companion.beckon_left = BECKON_SECONDS;
                match now.next {
                    Some(_) => companion.say("That woke another console further along the road. Follow my beam, let's see what it wants."),
                    None => {
                        for line in &game.chapter.closing {
                            companion.say_spoken(line, true);
                        }
                    }
                }
            }
            let guiding_ruler = companion.dialogue.as_ref().is_some_and(|d| matches!(d.stage, DialogueStage::Guide(_)));
            if !guiding_ruler && !ui.beam_open && focus_kind == PuzzleKind::Rail && !game.solved(focus) {
                if now.ruler_active && !before.ruler_active {
                    companion.say("Click the yellow pin I'm pointing at.");
                } else if now.ruler_active && now.ruler_first && !before.ruler_first && ruler.second.is_none() {
                    companion.say("And now this one.");
                }
            }
            if now.asking && !before.asking {
                companion.feel(Mood::Thinking, f32::INFINITY);
            } else if !now.asking && before.asking && companion.mood == Mood::Thinking {
                companion.feel(Mood::Neutral, 0.0);
            }
        }
    }
    companion.last_seen = Some(now);

    if std::mem::take(&mut ui.request_briefing) {
        if let Some(p) = ui.station {
            companion.open_dialogue(Console::Station(p), &game.def(p).dialogue, false, false, None);
        } else if ui.beam_open
            && let Some(side) = game.chapter.side(crate::beam::BEAM_ID)
        {
            companion.open_dialogue(Console::Beam, &side.dialogue, false, false, None);
        }
    }
    if std::mem::take(&mut ui.request_experiment)
        && let Some(p) = ui.station
        && let Some(demo) = game.def(p).demo.clone()
    {
        companion.open_experiment(p, &game.def(p).dialogue, demo, false);
    }
    // The console's `Replay experiment` button has queued a run: its reveal opens when it ends.
    if std::mem::take(&mut ui.request_replay)
        && let Some(p) = ui.station
    {
        companion.awaiting_demo = Some(AwaitingDemo { puzzle: p, hide_panel: false, first_run: false, started: false });
    }

    // Once the player opens the handbook (by pressing H or clicking its header), mark the first-time
    // handbook gate done and clear the prompt.
    if ui.handbook_open {
        companion.handbook_opened = true;
        if companion.handbook_prompt.take().is_some() {
            companion.feel(Mood::Pleased, 2.5);
        }
    }

    let testing = sim.testing().map(|p| game.def(p).kind);
    let running = testing.is_some() || sim.demo().is_some() || sim.measuring().is_some() || beam.animating();

    // Dismiss station dialogue when leaving its console, running a test/experiment, measuring with
    // the ruler (except during Station 0's hands-on ruler guide), or opening the full chat panel.
    let active_console = ui.station.map(Console::Station).or(ui.beam_open.then_some(Console::Beam));
    let guiding_ruler = companion.dialogue.as_ref().is_some_and(|d| matches!(d.stage, DialogueStage::Guide(_)));
    if companion.dialogue.as_ref().is_some_and(|d| Some(d.console) != active_console) || running || (ruler.active && !guiding_ruler) || ui.tutor_open {
        // If the player left mid-guide before finishing their first measurement, let the guide
        // resume when they reopen Station 0.
        if let Some(d) = &companion.dialogue
            && (matches!(d.stage, DialogueStage::Guide(_)) || matches!(d.pending, Some(NextBeat::MeasureGuide)))
            && game.save.notebook.entries.is_empty()
        {
            companion.presented.remove(&d.console);
        }
        companion.hand_over_handbook_gate();
        companion.dialogue = None;
    }
    if companion.awaiting_demo.is_some_and(|a| ui.station != Some(a.puzzle)) || ruler.active || ui.tutor_open {
        companion.awaiting_demo = None;
    }
    companion.track_awaiting_demo(sim.request.is_some(), sim.demo());

    // Lumen's initial briefing plays once per untouched console per session (hiding the right-hand
    // console panel until finished), as soon as nothing else is going on there.
    let free = companion.dialogue.is_none()
        && companion.awaiting_demo.is_none()
        && !running
        && !ruler.active
        && !ui.tutor_open
        && !waking.playing()
        && !companion.telling();
    if free {
        if let Some(p) = ui.station
            && should_auto_present_station(&game, p)
            && companion.presented.insert(Console::Station(p))
        {
            let pending = if game.def(p).kind == PuzzleKind::Rail && game.save.notebook.entries.is_empty() {
                Some(NextBeat::MeasureGuide)
            } else {
                game.def(p).demo.clone().map(|demo| NextBeat::Experiment(p, demo))
            };
            companion.open_dialogue(Console::Station(p), &game.def(p).dialogue, true, true, pending);
        } else if ui.beam_open
            && should_auto_present_beam(&game)
            && let Some(side) = game.chapter.side(crate::beam::BEAM_ID)
            && companion.presented.insert(Console::Beam)
        {
            companion.open_dialogue(Console::Beam, &side.dialogue, true, true, None);
        }
    }

    // Advance Station 0's action-gated ruler and notebook walkthrough from live game state.
    companion.sync_measure_guide(&ruler, !game.save.notebook.entries.is_empty(), ui.notebook_open);

    // Time passes for held looks, moods, dialogue fade-in and timed lines.
    if let Some((_, left)) = &mut companion.held {
        *left -= dt;
        if *left <= 0.0 {
            companion.held = None;
        }
    }
    if !companion.telling() {
        companion.beckon_left = (companion.beckon_left - dt).max(0.0);
    }
    companion.since_spoke += dt;
    companion.mood_left -= dt;
    if companion.mood_left <= 0.0 && companion.mood != Mood::Neutral {
        companion.mood = Mood::Neutral;
    }
    if let Some(dialogue) = &mut companion.dialogue {
        dialogue.shown += dt;
    }
    if companion.dialogue.is_none()
        && companion.lines.is_empty()
        && !running
        && !waking.playing()
        && !ui.tutor_open
        && let Some(prompt) = &mut companion.handbook_prompt
    {
        prompt.shown += dt;
    }
    if let Some(line) = companion.lines.first_mut() {
        line.shown += dt;
        if !line.wait {
            line.left -= dt;
            if line.left <= 0.0 {
                companion.lines.remove(0);
            }
        }
    }

    // A remark about where the player is, if nothing else is going on.
    let calm = Quiet {
        station_open: ui.at_console(),
        running,
        ruler: ruler.active || ruler.in_use(),
        conversation_open: ui.tutor_open,
        carrying: carry.holding.is_some(),
        speaking: !companion.lines.is_empty() || companion.dialogue.is_some() || companion.handbook_prompt.is_some(),
        since_spoke: companion.since_spoke,
    };
    if quiet(&calm) {
        let here = player.translation.xz();
        let due = game.chapter.remarks.iter().enumerate().find(|(i, remark)| {
            !companion.remarked.contains(i)
                && remark_due(&remark.when, &game)
                && landmarks.position(&remark.near).is_some_and(|at| at.xz().distance(here) <= remark.radius)
        });
        if let Some((i, remark)) = due
            && companion.remark(&remark.line)
        {
            companion.remarked.insert(i);
        }
    }

    if testing.is_some() && !asking {
        companion.feel(Mood::Thinking, 0.3);
    }
    let facts = Facts {
        focus_kind: (!ui.beam_open).then_some(focus_kind),
        station_open: ui.at_console(),
        focus_solved: game.solved(focus),
        next,
        next_station: next.map(|n| station_name(&game, n)),
        running,
        ruler_active: ruler.active,
        ruler_first: ruler.first.as_ref().filter(|_| ruler.second.is_none()).map(|p| p.label.clone()),
        beckon: companion.beckon_left > 0.0,
        at_finished_station: ui.near_station.is_some_and(|n| game.solved(n) || !game.unlocked(n)),
        held: companion.held.as_ref().map(|(place, _)| place.clone()),
        dialogue_look: companion.dialogue.as_ref().and_then(|d| d.look()).map(str::to_string),
        line_look: companion.lines.first().filter(|l| l.shown > 0.0).and_then(|l| l.look.clone()),
        carrying: carry.holding,
        light_running: waking.light_running(),
        slot_empty: carry.holds(Slot::Cart) == 0,
    };
    companion.gaze = choose_gaze(&facts).filter(|place| landmarks.position(place).is_some());

    // While the story names a place, the camera turns to show it past the avatar, and turns back
    // once it moves on.
    let story = companion.lines.first().filter(|l| l.keep && l.shown > 0.0).and_then(|l| l.look.clone());
    let show = story.filter(|_| !ui.at_console() && !waking.playing()).and_then(|place| Some((landmarks.position(&place)?, place)));
    match show {
        Some((at, place)) => {
            if companion.showing.as_ref() != Some(&place) {
                orbit.face(player.translation, at, SHOW_PITCH, SHOW_DISTANCE);
                companion.showing = Some(place);
            }
        }
        None => {
            if companion.showing.take().is_some() && !ui.at_console() && !waking.playing() {
                orbit.ease_home();
            }
        }
    }
}

/// After a test: the station's line on a pass; on a miss, where to look, and where to read or ask.
/// `ask_me`: the tutor is online and the player has not talked to it about this puzzle.
fn react_to_result(companion: &mut Companion, game: &Game, ui: &Ui, puzzle: usize, ask_me: bool) {
    let Some(outcome) = &ui.results[puzzle] else { return };
    let kind = game.def(puzzle).kind;
    if outcome.pass {
        companion.feel(Mood::Pleased, 5.0);
        let done = &game.def(puzzle).done;
        companion.say(if done.is_empty() { "Right on the mark!" } else { done.as_str() });
    } else {
        companion.feel(Mood::Concerned, 5.0);
        companion.say(match kind {
            PuzzleKind::Gravity => "Not quite. Have a look at how your prediction compares with the real fall.",
            PuzzleKind::Chase => "Not quite. Look at where the cart caught the crawler, and where the marker is.",
            _ => "Not this time. Look at where it ended up, and where the target is.",
        });
        let def = game.def(puzzle);
        // The shots after the drop tower are worked out with the player's own g, which passed there.
        if matches!(kind, PuzzleKind::Channel | PuzzleKind::Ridge) && companion.nudged.insert((puzzle, Nudge::YourG)) {
            companion.say("Your g worked at the drop tower, so I'd check the other readings first.");
            return;
        }
        let misses = game.save.progress(&def.id).attempts.iter().filter(|a| !a.pass).count();
        if ask_me && misses >= 2 && companion.nudged.insert((puzzle, Nudge::AskMe)) {
            companion.say("That's two misses. Want to talk it through? Press T. I won't give you the number, but I can help you find it.");
            return;
        }
        // Point back to the idea the station is about: its page, or the experiment that writes it.
        if def.demo.is_some()
            && let Some(page) = game.chapter.pages_found_at(&def.id).last()
        {
            if game.page_found(&page.id) {
                companion.say(format!("It might help to reread {} in the handbook. Press H.", page.title));
            } else {
                companion.say("The experiment at this station shows what's going on here. Give it a try.");
            }
        }
    }
}

/// After a release of the beam: glad when it holds, otherwise which end went down. The last load's
/// hold is answered by the wake instead, and a run of misses by a suggestion to look for a rule.
fn react_to_release(companion: &mut Companion, game: &Game, beam: &crate::beam::Beam) {
    let Some(swing) = beam.last else { return };
    let held = crate::beam::held(game) as usize;
    if swing.holds() {
        companion.beam_misses = 0;
        if crate::beam::woken(game) {
            companion.feel(Mood::Pleased, 3.0);
            companion.say("Level again. It still answers.");
        } else if held < physics_core::beam::CASES {
            companion.feel(Mood::Pleased, 5.0);
            companion.say("It's holding! And the machine's setting out a new load already.");
        }
        return;
    }
    companion.beam_misses += 1;
    companion.feel(Mood::Concerned, 3.0);
    if companion.beam_misses >= 4 && held + 1 == physics_core::beam::CASES && !companion.beam_rule {
        companion.beam_rule = true;
        companion.say("Before you carry anything else, look back at the loads that held. What did they have in common? If you can say it as a rule, it'll tell you what to try.");
        return;
    }
    companion.say(match swing {
        physics_core::beam::Swing::LoadDown => "The load's end went down. Have a look at what's on each arm.",
        _ => "The far end went down this time, so that side has too much.",
    });
}

/// Whether opening `puzzle`'s station should auto-play Lumen's full briefing (with the console
/// panel held back): only when the puzzle has not yet been solved, experimented on, or attempted.
fn should_auto_present_station(game: &Game, puzzle: usize) -> bool {
    let def = game.def(puzzle);
    !game.solved(puzzle) && !game.save.demo_watched(&def.id) && game.save.progress(&def.id).attempts.is_empty()
}

/// Whether opening the beam console should auto-play Lumen's briefing: only before any load has held.
fn should_auto_present_beam(game: &Game) -> bool {
    !crate::beam::woken(game) && crate::beam::held(game) == 0
}

/// After an experiment finishes at an open station: open Lumen's reveal bubble with mood, verdict,
/// full explanation, newly written handbook pages, and follow-up station topics.
fn open_demo_reveal(companion: &mut Companion, game: &Game, puzzle: usize, new_pages: Vec<(String, String)>, hide_panel: bool, show_verdict: bool) {
    let def = game.def(puzzle);
    let Some(demo) = &def.demo else { return };
    let verdict = if show_verdict {
        match game.save.first_guess(&def.id) {
            Some(guess) if guess == demo.answer => {
                companion.feel(Mood::Pleased, 5.0);
                Some(("You called it!".to_string(), true))
            }
            Some(_) => {
                companion.feel(Mood::Concerned, 4.0);
                Some(("Not what you expected? Don't worry, this one surprises most people.".to_string(), false))
            }
            None => None,
        }
    } else {
        None
    };
    companion.open_demo_reveal(puzzle, &def.dialogue, verdict, demo.reveal.clone(), new_pages, hide_panel);
}

/// After an experiment's first run away from a console: how the guess went, then what was seen.
fn react_to_demo(companion: &mut Companion, game: &Game, puzzle: usize) {
    let def = game.def(puzzle);
    let Some(demo) = &def.demo else { return };
    match game.save.first_guess(&def.id) {
        Some(guess) if guess == demo.answer => {
            companion.feel(Mood::Pleased, 5.0);
            companion.say("You called it!");
        }
        Some(_) => {
            companion.feel(Mood::Concerned, 4.0);
            companion.say("Not what you expected? Don't worry, this one surprises most people.");
        }
        None => {}
    }
    companion.say(first_sentence(&demo.reveal));
}

/// Lumen's line the first time a handbook page is written, kept up until the handbook is opened.
fn first_page_prompt(title: &str) -> String {
    format!("There's a new page in your handbook, {title}. Press H to open it and have a look.")
}

fn first_sentence(text: &str) -> &str {
    text.split_inclusive(". ").next().unwrap_or(text).trim()
}

fn lowercase_first(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => String::new(),
    }
}

// --- Drawing ---------------------------------------------------------------------------------

/// Radius of the eye, in interface points.
const EYE: f32 = 26.0;
/// Distance of the eye's centre from the left and bottom edges of the screen.
const EYE_INSET: EVec2 = vec2(58.0, 112.0);
/// How far the eye rises to float above the ruler's panel while the ruler is out.
const RULER_LIFT: f32 = 140.0;
/// How far below its centre the eye reaches on screen: its glow, and how far it bobs.
const EYE_REACH: f32 = EYE + 18.0 + 3.0;
/// The least gap between the eye and the top of the letterbox's lower bar.
const BAR_CLEARANCE: f32 = 12.0;
/// How much higher the eye floats while it speaks at the bottom centre, clear of the hints there.
const FORWARD_RISE: f32 = 30.0;
/// How long the eye takes to come forward or go back, in seconds.
const FORWARD_SECONDS: f32 = 0.45;
/// How long the eye stays forward after its last line, so lines in a row do not send it back and forth.
const FORWARD_LINGER: f32 = 0.6;
/// How long the ring that marks a new line takes to spread and fade, in seconds.
const RING_SECONDS: f32 = 0.7;
/// Width of the station's panel at the right, with its margin: the eye centres on the rest.
const STATION_PANEL: f32 = 420.0;
/// Font size of a line in its bubble, and the width it wraps at.
const LINE_FONT: f32 = 15.0;
const LINE_WRAP: f32 = 320.0;

#[derive(Default)]
struct Animation {
    clock: f32,
    next_blink: f32,
    blink: f32,
    /// Smoothed offset of the iris from the eye's centre, as a fraction of the eye.
    iris: EVec2,
    /// How far the eye has risen, in points.
    lift: f32,
    /// How far the eye has come from its corner toward the bottom centre, from 0 to 1, before easing.
    forward: f32,
    /// Seconds the eye stays forward after it last had something to say.
    linger: f32,
    /// Smoothed end of the beam on screen, and how visible the beam is.
    beam_end: Option<Pos2>,
    beam: f32,
    colour: [f32; 3],
}

/// Where the eye rests, before it bobs: in its corner at `forward` 0, and at the bottom centre of the
/// screen left of `right_edge` (what the station's panel leaves free) at 1, `lift` points higher.
fn eye_place(screen: egui::Rect, lift: f32, forward: f32, right_edge: f32) -> Pos2 {
    let corner = pos2(screen.left() + EYE_INSET.x, screen.bottom() - EYE_INSET.y - lift);
    let speaking = pos2((screen.left() + right_edge) / 2.0, corner.y - FORWARD_RISE);
    corner.lerp(speaking, forward.clamp(0.0, 1.0))
}

/// How far the eye rises: above the ruler's panel while the ruler is out, and above the lower
/// letterbox bar, `bar` points tall, while a wake plays.
fn lift_for(bar: f32, ruler: bool) -> f32 {
    let over_bar = if bar > 0.0 { (bar + BAR_CLEARANCE - (EYE_INSET.y - EYE_REACH)).max(0.0) } else { 0.0 };
    over_bar.max(if ruler { RULER_LIFT } else { 0.0 })
}

/// Bottom left corner of the panel that grows out of the eye.
fn bubble_anchor(eye: Pos2) -> Pos2 {
    eye + vec2(-EYE + 2.0, -EYE - 22.0)
}

/// Where the bubble of a line `size` big goes over the eye: off to its right in the corner, centred
/// over it at the bottom centre, and always on the screen.
fn bubble_rect(screen: egui::Rect, eye: Pos2, size: EVec2, forward: f32) -> egui::Rect {
    let corner = bubble_anchor(eye).x;
    let centred = eye.x - size.x / 2.0;
    let x = corner + (centred - corner) * forward.clamp(0.0, 1.0);
    let x = x.min(screen.right() - 8.0 - size.x).max(screen.left() + 8.0);
    egui::Rect::from_min_size(pos2(x, eye.y - EYE - 22.0 - size.y), size)
}

const BUBBLE_FILL: Color32 = Color32::from_rgba_premultiplied(11, 20, 31, 235);

/// The tail that joins a bubble or panel to the eye, from the bubble's lower edge over the eye.
fn paint_tail(painter: &egui::Painter, rect: egui::Rect, eye: Pos2, fill: Color32, stroke: Stroke) {
    let base = eye.x.clamp(rect.left() + 26.0, (rect.right() - 26.0).max(rect.left() + 26.0));
    let tail = [pos2(base - 8.0, rect.bottom() - 1.0), pos2(base + 8.0, rect.bottom() - 1.0), pos2(eye.x - 4.0, eye.y - EYE - 6.0)];
    painter.add(egui::Shape::convex_polygon(tail.to_vec(), fill, Stroke::NONE));
    painter.line_segment([tail[0], tail[2]], stroke);
    painter.line_segment([tail[1], tail[2]], stroke);
}

fn mood_colour(mood: Mood) -> [f32; 3] {
    match mood {
        Mood::Neutral => [90.0, 215.0, 255.0],
        Mood::Pleased => [120.0, 240.0, 170.0],
        Mood::Concerned => [255.0, 185.0, 90.0],
        Mood::Thinking => [175.0, 165.0, 255.0],
    }
}

fn tint(c: [f32; 3], alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0] as u8, c[1] as u8, c[2] as u8, (alpha.clamp(0.0, 1.0) * 255.0) as u8)
}

/// Where a world point is on screen, in interface points. `Err` carries the direction to it from
/// the middle of the screen when it is behind the camera or outside the screen.
fn on_screen(camera: &Camera, at: &GlobalTransform, world: Vec3, zoom: f32, screen: egui::Rect) -> Result<Pos2, EVec2> {
    let view = at.affine().inverse().transform_point3(world);
    if view.z < -0.2
        && let Ok(p) = camera.world_to_viewport(at, world)
    {
        let p = pos2(p.x / zoom, p.y / zoom);
        if screen.shrink(12.0).contains(p) {
            return Ok(p);
        }
        return Err((p - screen.center()).normalized());
    }
    // Behind: left and right still hold; lean the arrow toward the bottom edge.
    let d = vec2(view.x, (-view.y).max(0.0) + 0.3 * view.x.abs().max(0.1));
    Err(if d.length() < 1e-3 { vec2(0.0, 1.0) } else { d.normalized() })
}

/// The point where a ray from the middle of `rect` in direction `d` leaves it.
fn edge_point(rect: egui::Rect, d: EVec2) -> Pos2 {
    let half = rect.size() / 2.0;
    let sx = if d.x.abs() > 1e-4 { half.x / d.x.abs() } else { f32::INFINITY };
    let sy = if d.y.abs() > 1e-4 { half.y / d.y.abs() } else { f32::INFINITY };
    rect.center() + d * sx.min(sy)
}

/// Clamp `rect` inside `bounds` by translating it without changing its size.
fn clamp_inside(rect: egui::Rect, bounds: egui::Rect) -> egui::Rect {
    rect.translate(vec2(
        (bounds.left() - rect.left()).max(0.0) - (rect.right() - bounds.right()).max(0.0),
        (bounds.top() - rect.top()).max(0.0) - (rect.bottom() - bounds.bottom()).max(0.0),
    ))
}

/// Total area of `rect` overlapping any of `occupied` (padded by a small margin).
fn overlap_area(rect: egui::Rect, occupied: &[egui::Rect]) -> f32 {
    occupied
        .iter()
        .map(|panel| {
            let inter = rect.intersect(panel.expand(4.0));
            if inter.is_positive() { inter.width() * inter.height() } else { 0.0 }
        })
        .sum()
}

/// Place a floating landmark label near `end`, keeping it on screen and clear of open UI panels.
fn place_label(screen: egui::Rect, end: Pos2, size: EVec2, edge: Option<EVec2>, occupied: &[egui::Rect]) -> egui::Rect {
    let bounds = screen.shrink(4.0);
    let mut candidates = Vec::new();
    match edge {
        Some(d) => {
            let side = vec2(-d.y, d.x);
            candidates.push(clamp_inside(egui::Rect::from_center_size(end - d * 34.0, size), bounds));
            candidates.push(clamp_inside(egui::Rect::from_center_size(end - d * 28.0 + side * (size.x / 2.0 + 14.0), size), bounds));
            candidates.push(clamp_inside(egui::Rect::from_center_size(end - d * 28.0 - side * (size.x / 2.0 + 14.0), size), bounds));
        }
        None => {
            for offset in [
                vec2(0.0, -26.0),
                vec2(0.0, 26.0),
                vec2(size.x / 2.0 + 18.0, 0.0),
                vec2(-(size.x / 2.0 + 18.0), 0.0),
                vec2(size.x / 2.0 + 18.0, -26.0),
                vec2(size.x / 2.0 + 18.0, 26.0),
            ] {
                candidates.push(clamp_inside(egui::Rect::from_center_size(end + offset, size), bounds));
            }
        }
    }
    let first = candidates[0];
    // If the preferred position hits a panel (such as the top-left dock), also consider sliding
    // just below or beside that panel.
    for panel in occupied {
        if first.intersects(panel.expand(4.0)) {
            candidates.push(clamp_inside(egui::Rect::from_min_size(pos2(first.left(), panel.bottom() + 8.0), size), bounds));
            candidates.push(clamp_inside(egui::Rect::from_min_size(pos2(panel.right() + 8.0, first.top()), size), bounds));
            candidates.push(clamp_inside(egui::Rect::from_min_size(pos2(panel.left() - 8.0 - size.x, first.top()), size), bounds));
        }
    }
    candidates.into_iter().min_by(|a, b| overlap_area(*a, occupied).total_cmp(&overlap_area(*b, occupied))).unwrap_or(first)
}

#[allow(clippy::too_many_arguments)]
fn draw(
    mut contexts: EguiContexts,
    time: Res<Time>,
    landmarks: Res<Landmarks>,
    game: Res<Game>,
    mut lumen: ResMut<Lumen>,
    mut companion: ResMut<Companion>,
    mut ui_state: ResMut<Ui>,
    ruler: Res<Ruler>,
    mut sim: ResMut<Sim>,
    beam: Res<crate::beam::Beam>,
    waking: Res<crate::waking::Waking>,
    orbit: Res<crate::player::OrbitCamera>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    if ui_state.paused {
        return Ok(());
    }
    let dt = time.delta_secs().min(0.1);
    let companion = &mut *companion;
    let mood = companion.mood;
    let gaze = companion.gaze.clone();
    let panel_hidden = companion.hiding_console_panel();
    let a = &mut companion.anim;
    a.clock += dt;
    let t = a.clock;

    // Colour drifts toward the mood's.
    let goal = mood_colour(mood);
    if a.colour == [0.0; 3] {
        a.colour = goal;
    }
    for (c, g) in a.colour.iter_mut().zip(goal) {
        *c += (g - *c) * (dt * 4.0).min(1.0);
    }
    let colour = a.colour;

    // Blinking.
    if a.next_blink <= 0.0 {
        a.next_blink = t + 2.5;
    }
    if t >= a.next_blink {
        a.blink += dt / 0.16;
        if a.blink >= 1.0 {
            a.blink = 0.0;
            // Pseudo-random gaps of 2.5 to 6 seconds.
            a.next_blink = t + 2.5 + 3.5 * (t * 12.9898).sin().abs().fract();
        }
    }
    let closure = if t >= a.next_blink { 1.0 - (a.blink * 2.0 - 1.0).abs() } else { 0.0 };

    let screen = ctx.content_rect();
    let zoom = ctx.zoom_factor();
    // Above the ruler's panel, and above the letterbox while a wake plays.
    let bar = waking.play.map_or(0.0, |play| crate::cinematic::letterbox_height(screen, play.timeline.bars(play.t)));
    let lift = lift_for(bar, ruler.in_use());
    a.lift += (lift - a.lift) * (dt * 6.0).min(1.0);
    // The bars come in fast: the eye keeps up with them, so it is never under one.
    a.lift = a.lift.max(lift_for(bar, false));
    let hud_hidden = crate::ui::hud_hidden(&waking, &sim, &beam);
    let show_chat = ui_state.tutor_open && !hud_hidden;
    // Forward to the bottom centre while there is something to say, back to the corner after.
    let speaking = !show_chat && (companion.dialogue.is_some() || !companion.lines.is_empty() || companion.handbook_prompt.is_some());
    a.linger = if speaking { FORWARD_LINGER } else { (a.linger - dt).max(0.0) };
    let toward = if speaking || (a.linger > 0.0 && !show_chat) { 1.0 } else { 0.0 };
    a.forward += (toward - a.forward).clamp(-dt / FORWARD_SECONDS, dt / FORWARD_SECONDS);
    let forward = crate::voxel::smoothstep(0.0, 1.0, a.forward);
    let panel_shown = ui_state.at_console() && !panel_hidden;
    let right_edge = if panel_shown { screen.right() - STATION_PANEL } else { screen.right() };
    let rest = eye_place(screen, a.lift, forward, right_edge);
    let centre = rest + vec2(0.0, (t * 1.7).sin() * 3.0);
    let (camera, at) = *camera;

    // Where the beam goes: a point on screen, or an arrow at the screen's edge.
    let target = gaze.as_deref().and_then(|place| landmarks.position(place).map(|p| (place, p)));
    let aim = target.map(|(place, world)| match on_screen(camera, at, world, zoom, screen) {
        Ok(p) => (place, p, None),
        Err(d) => (place, edge_point(screen.shrink(26.0), d), Some(d)),
    });
    a.beam = (a.beam + if aim.is_some() { dt * 3.0 } else { -dt * 3.0 }).clamp(0.0, 1.0);
    if let Some((_, end, _)) = aim {
        a.beam_end = Some(match a.beam_end {
            Some(old) if a.beam > 0.05 => old + (end - old) * (dt * 8.0).min(1.0),
            _ => end,
        });
    }

    // The iris looks toward the beam's end, or wanders when there is nothing to look at.
    let look = match (aim, a.beam_end) {
        (Some(_), Some(end)) => {
            let d = end - centre;
            d / d.length().max(1.0) * (d.length() / 140.0).min(1.0)
        }
        _ => vec2((t * 0.37).sin() * 0.5, (t * 0.23).cos() * 0.25),
    };
    let look = if mood == Mood::Thinking { vec2(look.x * 0.4 + (t * 0.9).sin() * 0.3, -0.6) } else { look };
    a.iris += (look - a.iris) * (dt * 7.0).min(1.0);
    let iris_at = centre + a.iris * EYE * 0.42;

    // The beam, under the windows.
    if a.beam > 0.0
        && let Some(end) = a.beam_end
    {
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("lumen-beam")));
        let start = iris_at + (end - iris_at).normalized() * EYE * 0.55;
        let steps = 32;
        for i in 0..steps {
            let (s0, s1) = (i as f32 / steps as f32, (i + 1) as f32 / steps as f32);
            let fade = 0.45 + 0.45 * ((s0 - 0.5).abs() * 2.0).powi(2);
            painter.line_segment([start.lerp(end, s0), start.lerp(end, s1)], Stroke::new(5.0, tint(colour, 0.12 * a.beam)));
            painter.line_segment([start.lerp(end, s0), start.lerp(end, s1)], Stroke::new(2.0, tint(colour, fade * a.beam)));
        }
        // Sparks running along the beam.
        for k in 0..4 {
            let s = (t * 0.55 + k as f32 / 4.0).fract();
            painter.circle_filled(start.lerp(end, s), 2.6, tint([235.0, 250.0, 255.0], 0.85 * a.beam));
        }
        let edge = aim.and_then(|(_, _, d)| d);
        match edge {
            None => {
                let pulse = 9.0 + 3.0 * (t * 4.0).sin();
                painter.circle_stroke(end, pulse, Stroke::new(2.0, tint(colour, 0.9 * a.beam)));
                painter.circle_filled(end, 3.0, tint(colour, a.beam));
            }
            Some(d) => {
                // An arrow at the edge of the screen: it is that way.
                let side = vec2(-d.y, d.x);
                let tip = end + d * (6.0 + 3.0 * (t * 5.0).sin());
                let points = vec![tip + d * 12.0, tip - d * 6.0 + side * 10.0, tip - d * 6.0 - side * 10.0];
                painter.add(egui::Shape::convex_polygon(points, tint(colour, 0.9 * a.beam), Stroke::NONE));
            }
        }
        if let Some((place, _, d)) = aim {
            let galley = painter.layout_no_wrap(place.to_string(), egui::FontId::proportional(12.0), tint([235.0, 245.0, 255.0], a.beam));
            let size = galley.size() + vec2(12.0, 6.0);
            let occupied: Vec<egui::Rect> = ui_state.occupied.iter().map(|r| egui::Rect::from_min_max(pos2(r[0], r[1]), pos2(r[2], r[3]))).collect();
            let rect = place_label(screen, end, size, d, &occupied);
            painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(12, 22, 34, (200.0 * a.beam) as u8));
            painter.rect_stroke(rect, 4.0, Stroke::new(1.0, tint(colour, 0.6 * a.beam)), egui::StrokeKind::Inside);
            painter.galley(rect.min + vec2(6.0, 3.0), galley, Color32::WHITE);
        }
    }

    // The eye itself, which can be clicked to talk when the cursor is visible and HUD is shown.
    let interactive = !orbit.free_look && !hud_hidden;
    let reach = EYE + 16.0;
    let area = egui::Area::new(egui::Id::new("lumen-eye")).order(egui::Order::Foreground).fixed_pos(centre - vec2(reach, reach)).show(&ctx, |ui| {
        let sense = if interactive { egui::Sense::click() } else { egui::Sense::empty() };
        let (rect, response) = ui.allocate_exact_size(vec2(reach, reach) * 2.0, sense);
        paint_eye(ui.painter(), rect.center(), colour, mood, closure, a.iris, t, interactive && response.hovered());
        if interactive { response.on_hover_text("Lumen. Click here or press T to talk to me.") } else { response }
    });
    if interactive && area.inner.clicked() {
        ui_state.tutor_open = !ui_state.tutor_open;
    }

    if show_chat {
        // The conversation, in a panel that grows out of the eye where its lines appear. It moves
        // with the eye and cannot be dragged away from it.
        let anchor = bubble_anchor(rest);
        let room = (anchor.y - ui_state.dock_bottom - 140.0).clamp(60.0, 300.0);
        let stroke = Stroke::new(1.5, tint(colour, 0.8));
        let panel =
            egui::Area::new(egui::Id::new("lumen-panel")).order(egui::Order::Foreground).pivot(egui::Align2::LEFT_BOTTOM).fixed_pos(anchor).show(&ctx, |ui| {
                egui::Frame::new().fill(BUBBLE_FILL).stroke(stroke).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
                    if crate::ui::dock_header(ui, "Lumen", "T", true, None) {
                        ui_state.tutor_open = false;
                    }
                    ui.separator();
                    crate::ui::tutor_body(ui, &game, &mut ui_state, &mut lumen, &landmarks, &companion.history, room);
                });
            });
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("lumen-panel-tail")));
        paint_tail(&painter, panel.response.rect, centre, BUBBLE_FILL, stroke);
    } else if !hud_hidden && companion.dialogue.is_some() {
        // Interactive station briefing, action-gated Station 0 guide, "Try it first" experiment
        // prompt, post-experiment reveal, and Q&A chips in a bubble above the eye.
        let (shown, console, topics, hide_panel, has_more, next_label, stage) = {
            let d = companion.dialogue.as_ref().expect("checked above");
            let next_label = match (&d.stage, &d.pending) {
                (DialogueStage::Lines(lines), Some(NextBeat::MeasureGuide)) if lines.len() == 1 => "Measure it ▸",
                (DialogueStage::Lines(lines), Some(NextBeat::Experiment(..))) if lines.len() == 1 => "Try it first ▸",
                _ => "Next ▸",
            };
            (d.shown, d.console, d.topics.clone(), d.hide_panel, d.has_more(), next_label, d.stage.clone())
        };
        let alpha = (shown / 0.25).min(1.0);
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("lumen-dialogue-ring")));
        if shown < RING_SECONDS {
            let k = shown / RING_SECONDS;
            painter.circle_stroke(centre, EYE + 10.0 + 46.0 * k, Stroke::new(2.5 * (1.0 - k) + 0.5, tint(colour, 0.8 * (1.0 - k))));
        }
        let preferred_w: f32 = if matches!(stage, DialogueStage::Lines(_) | DialogueStage::Guide(_)) { 380.0 } else { 440.0 };
        let max_w = preferred_w.min((right_edge - screen.left() - 24.0).max(240.0));
        let anchor_x = centre.x.clamp(screen.left() + max_w / 2.0 + 10.0, (right_edge - max_w / 2.0 - 10.0).max(screen.left() + max_w / 2.0 + 10.0));
        let anchor = pos2(anchor_x, centre.y - EYE - 22.0);
        let fill = BUBBLE_FILL.gamma_multiply(alpha);
        let stroke = Stroke::new(1.5, tint(colour, 0.85 * alpha));
        let mut advance = false;
        let mut pick: Option<usize> = None;
        let mut run_guess: Option<(usize, usize)> = None;
        let mut replay_demo: Option<usize> = None;
        let mut open_page: Option<String> = None;
        let mut dismiss = false;
        let mut open_tutor = false;
        let dismiss_label = if hide_panel { "Open console" } else { "Got it" };
        let handbook_gate = companion.reveal_waits_for_handbook();
        let area =
            egui::Area::new(egui::Id::new("lumen-dialogue")).order(egui::Order::Foreground).pivot(Align2::CENTER_BOTTOM).fixed_pos(anchor).show(&ctx, |ui| {
                egui::Frame::new().fill(fill).stroke(stroke).corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
                    ui.set_max_width(max_w);
                    match &stage {
                        DialogueStage::Lines(lines) => {
                            let text = lines.first().map_or("", |l| l.text.as_str());
                            let line_resp = ui.add(
                                egui::Label::new(RichText::new(text).size(LINE_FONT).color(tint([235.0, 245.0, 255.0], alpha))).wrap().sense(if has_more {
                                    egui::Sense::click()
                                } else {
                                    egui::Sense::hover()
                                }),
                            );
                            if has_more && line_resp.clicked() {
                                advance = true;
                            }
                            ui.add_space(8.0);
                            if has_more {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Click or press Enter").small().color(crate::ui::DIM));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button(RichText::new(next_label).strong().color(crate::ui::LUMEN)).clicked() {
                                            advance = true;
                                        }
                                    });
                                });
                            } else {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                                    for (idx, topic) in topics.iter().enumerate() {
                                        let asked = companion.asked.contains(&(console, idx));
                                        let label = if asked {
                                            RichText::new(format!("✔ {}", topic.ask)).color(crate::ui::DIM)
                                        } else {
                                            RichText::new(&topic.ask).color(crate::ui::LUMEN)
                                        };
                                        if ui.button(label).clicked() {
                                            pick = Some(idx);
                                        }
                                    }
                                    if lumen.tutor.is_online()
                                        && ui.button(RichText::new("Ask something else (T)").color(Color32::from_rgb(200, 225, 240))).clicked()
                                    {
                                        open_tutor = true;
                                    }
                                    if ui.button(RichText::new(dismiss_label).strong()).clicked() {
                                        dismiss = true;
                                    }
                                });
                            }
                        }
                        DialogueStage::Guide(step) => {
                            ui.add(egui::Label::new(RichText::new(step.text()).size(LINE_FONT).color(tint([235.0, 245.0, 255.0], alpha))).wrap());
                            ui.add_space(6.0);
                            ui.label(RichText::new(step.cue()).small().strong().color(crate::ui::WARN));
                        }
                        DialogueStage::Experiment { puzzle, demo } => {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Try it first").small().strong().color(crate::ui::LUMEN));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    ui.label(RichText::new("Free experiment").small().color(crate::ui::DIM));
                                });
                            });
                            ui.add_space(4.0);
                            ui.add(egui::Label::new(RichText::new(&demo.question).size(LINE_FONT).color(tint([235.0, 245.0, 255.0], alpha))).wrap());
                            ui.add_space(8.0);
                            ui.spacing_mut().item_spacing.y = 5.0;
                            let ready = !sim.busy();
                            for (idx, choice) in demo.choices.iter().enumerate() {
                                let button = egui::Button::new(RichText::new(choice).color(Color32::from_rgb(230, 242, 252))).wrap().min_size(vec2(max_w, 0.0));
                                if ui.add_enabled(ready, button).clicked() {
                                    run_guess = Some((*puzzle, idx));
                                }
                            }
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Pick a prediction to run it.").small().color(crate::ui::DIM));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button(RichText::new("Skip for now").small().color(crate::ui::DIM)).clicked() {
                                        dismiss = true;
                                    }
                                });
                            });
                        }
                        DialogueStage::Reveal { puzzle, verdict, reveal, new_pages } => {
                            if let Some((line, right)) = verdict {
                                let colour = if *right { crate::ui::GOOD } else { crate::ui::WARN };
                                ui.add(egui::Label::new(RichText::new(line).size(LINE_FONT).strong().color(colour)).wrap());
                                ui.add_space(4.0);
                            }
                            ui.add(egui::Label::new(RichText::new(reveal).size(LINE_FONT).color(tint([235.0, 245.0, 255.0], alpha))).wrap());
                            if !new_pages.is_empty() {
                                ui.add_space(6.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
                                    let heading = if new_pages.len() == 1 { "New handbook page:" } else { "New handbook pages:" };
                                    ui.label(RichText::new(heading).small().color(crate::ui::GOOD));
                                    for (id, title) in new_pages {
                                        if ui
                                            .small_button(RichText::new(title).color(crate::ui::GOOD))
                                            .on_hover_text("Open this page in the handbook")
                                            .clicked()
                                        {
                                            open_page = Some(id.clone());
                                        }
                                    }
                                });
                            }
                            ui.add_space(8.0);
                            if handbook_gate {
                                // The player's first pages: open the handbook before anything else.
                                ui.add(egui::Label::new(RichText::new(FIRST_PAGES_LINE).size(LINE_FONT).color(tint([235.0, 245.0, 255.0], alpha))).wrap());
                                ui.add_space(6.0);
                                ui.label(RichText::new("Press H or click the page").small().strong().color(crate::ui::WARN));
                            } else {
                                ui.horizontal_wrapped(|ui| {
                                    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                                    for (idx, topic) in topics.iter().enumerate() {
                                        let asked = companion.asked.contains(&(console, idx));
                                        let label = if asked {
                                            RichText::new(format!("✔ {}", topic.ask)).color(crate::ui::DIM)
                                        } else {
                                            RichText::new(&topic.ask).color(crate::ui::LUMEN)
                                        };
                                        if ui.button(label).clicked() {
                                            pick = Some(idx);
                                        }
                                    }
                                    if ui
                                        .add_enabled(!sim.busy(), egui::Button::new(RichText::new("Run it again").color(Color32::from_rgb(200, 225, 240))))
                                        .clicked()
                                    {
                                        replay_demo = Some(*puzzle);
                                    }
                                    if lumen.tutor.is_online()
                                        && ui.button(RichText::new("Ask something else (T)").color(Color32::from_rgb(200, 225, 240))).clicked()
                                    {
                                        open_tutor = true;
                                    }
                                    if ui.button(RichText::new(dismiss_label).strong()).clicked() {
                                        dismiss = true;
                                    }
                                });
                            }
                        }
                    }
                });
            });
        let tail_painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("lumen-dialogue-tail")));
        paint_tail(&tail_painter, area.response.rect, centre, fill, stroke);
        let r = area.response.rect;
        ui_state.occupied.push([r.min.x, r.min.y, r.max.x, r.max.y]);
        if let Some((puzzle, guess)) = run_guess {
            ui_state.guess[puzzle] = Some(guess);
            sim.request = Some(Request::Demo(puzzle));
            companion.awaiting_demo = Some(AwaitingDemo { puzzle, hide_panel, first_run: true, started: false });
            companion.dialogue = None;
        } else if let Some(puzzle) = replay_demo {
            sim.request = Some(Request::Demo(puzzle));
            companion.awaiting_demo = Some(AwaitingDemo { puzzle, hide_panel, first_run: false, started: false });
            companion.dialogue = None;
        } else if let Some(page) = open_page {
            companion.dialogue = None;
            ui_state.open_page(&page);
        } else if open_tutor {
            companion.dialogue = None;
            ui_state.tutor_open = true;
        } else if dismiss {
            companion.dialogue = None;
        } else if let Some(idx) = pick {
            companion.pick_topic(idx);
        } else if advance {
            companion.advance_dialogue();
        }
    } else if let Some(line) = companion.lines.first() {
        // What Lumen is saying, in a clickable bubble over the eye.
        let more = companion.lines.len() > 1;
        let alpha = if line.wait { (line.shown / 0.25).min(1.0) } else { (line.shown / 0.25).min(1.0) * (line.left / 0.5).min(1.0) };
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("lumen-says")));
        // A ring spreads from the eye as a line starts, to catch the eye of the player.
        if line.shown < RING_SECONDS {
            let k = line.shown / RING_SECONDS;
            painter.circle_stroke(centre, EYE + 10.0 + 46.0 * k, Stroke::new(2.5 * (1.0 - k) + 0.5, tint(colour, 0.8 * (1.0 - k))));
        }
        let galley = painter.layout(line.text.clone(), egui::FontId::proportional(LINE_FONT), tint([235.0, 245.0, 255.0], alpha), LINE_WRAP);
        let enter_advances = (!ui_state.at_console() || panel_hidden) && !waking.playing();
        let hint_text = match (orbit.free_look, enter_advances, more, line.wait) {
            (true, _, true, _) => "Press Enter for next",
            (true, _, false, true) => "Press Enter to continue",
            (true, _, false, false) => "Press Enter to dismiss",
            (false, true, true, _) => "Click or press Enter for next",
            (false, true, false, true) => "Click or press Enter to continue",
            (false, true, false, false) => "Click or press Enter to dismiss",
            (false, false, true, _) => "Click for next",
            (false, false, false, true) => "Click to continue",
            (false, false, false, false) => "Click to dismiss",
        };
        let hint_galley = painter.layout_no_wrap(hint_text.into(), egui::FontId::proportional(11.0), tint([160.0, 180.0, 200.0], alpha * 0.85));
        let width = galley.size().x.max(hint_galley.size().x);
        let size = vec2(width + 20.0, galley.size().y + hint_galley.size().y + 18.0);
        let rect = bubble_rect(screen, centre, size, forward);
        let hit = egui::Area::new(egui::Id::new("lumen-bubble")).order(egui::Order::Foreground).fixed_pos(rect.min).show(&ctx, |ui| {
            let sense = if orbit.free_look { egui::Sense::empty() } else { egui::Sense::click() };
            let (_, response) = ui.allocate_exact_size(rect.size(), sense);
            response
        });
        let hovered = !orbit.free_look && hit.inner.hovered();
        let fill = BUBBLE_FILL.gamma_multiply(alpha);
        let stroke = Stroke::new(if hovered { 2.0 } else { 1.5 }, tint(colour, (if hovered { 1.0 } else { 0.8 }) * alpha));
        painter.rect_filled(rect, 8.0, fill);
        painter.rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Inside);
        paint_tail(&painter, rect, centre, fill, stroke);
        painter.galley(rect.min + vec2(10.0, 7.0), galley, Color32::WHITE);
        painter.galley(pos2(rect.right() - 10.0 - hint_galley.size().x, rect.bottom() - 6.0 - hint_galley.size().y), hint_galley, Color32::WHITE);
        if !orbit.free_look && hit.inner.clicked() {
            companion.advance_line();
        }
    } else if !hud_hidden && let Some(prompt) = &companion.handbook_prompt {
        // Action-gated first handbook prompt: stays on screen until the player opens the handbook.
        let alpha = (prompt.shown / 0.25).min(1.0);
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("lumen-action-prompt")));
        if prompt.shown < RING_SECONDS {
            let k = prompt.shown / RING_SECONDS;
            painter.circle_stroke(centre, EYE + 10.0 + 46.0 * k, Stroke::new(2.5 * (1.0 - k) + 0.5, tint(colour, 0.8 * (1.0 - k))));
        }
        let galley = painter.layout(prompt.text.clone(), egui::FontId::proportional(LINE_FONT), tint([235.0, 245.0, 255.0], alpha), LINE_WRAP);
        let cue_galley = painter.layout_no_wrap("Press H or click Handbook".into(), egui::FontId::proportional(11.5), crate::ui::WARN.gamma_multiply(alpha));
        let width = galley.size().x.max(cue_galley.size().x);
        let size = vec2(width + 20.0, galley.size().y + cue_galley.size().y + 20.0);
        let rect = bubble_rect(screen, centre, size, forward);
        let fill = BUBBLE_FILL.gamma_multiply(alpha);
        let stroke = Stroke::new(1.5, tint(colour, 0.85 * alpha));
        painter.rect_filled(rect, 8.0, fill);
        painter.rect_stroke(rect, 8.0, stroke, egui::StrokeKind::Inside);
        paint_tail(&painter, rect, centre, fill, stroke);
        painter.galley(rect.min + vec2(10.0, 7.0), galley, Color32::WHITE);
        painter.galley(pos2(rect.left() + 10.0, rect.bottom() - 6.0 - cue_galley.size().y), cue_galley, Color32::WHITE);
        ui_state.occupied.push([rect.min.x, rect.min.y, rect.max.x, rect.max.y]);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn paint_eye(painter: &egui::Painter, c: Pos2, colour: [f32; 3], mood: Mood, closure: f32, iris: EVec2, t: f32, hovered: bool) {
    let r = EYE;
    // Glow.
    for i in 0..6 {
        let k = i as f32 / 6.0;
        painter.circle_filled(c, r + 4.0 + 14.0 * k, tint(colour, 0.06 * (1.0 - k) * if hovered { 1.8 } else { 1.0 }));
    }
    // A ring of dashes turning slowly, and a fainter full ring.
    painter.circle_stroke(c, r + 9.0, Stroke::new(1.0, tint(colour, 0.25)));
    for i in 0..10 {
        let a0 = t * 0.5 + i as f32 * std::f32::consts::TAU / 10.0;
        let points: Vec<Pos2> = (0..=5).map(|k| c + EVec2::angled(a0 + k as f32 * 0.07) * (r + 9.0)).collect();
        painter.add(egui::Shape::line(points, Stroke::new(2.0, tint(colour, 0.75))));
    }
    // The eyeball: dark glass with a bright rim.
    let glass = Color32::from_rgba_unmultiplied(10, 26, 40, 235);
    painter.circle_filled(c, r, glass);
    // Iris and pupil.
    let at = c + iris * r * 0.42;
    painter.circle_filled(at, r * 0.5, tint(colour, 0.35));
    painter.circle_stroke(at, r * 0.5, Stroke::new(2.0, tint(colour, 0.95)));
    painter.circle_filled(at, r * 0.22, Color32::from_rgb(8, 16, 26));
    painter.circle_filled(at + vec2(-r * 0.14, -r * 0.16), r * 0.08, Color32::from_rgba_unmultiplied(255, 255, 255, 200));

    // Lids, as thin vertical strips so their edges can take any curve.
    let (mut upper, mut lower, mut slant) = match mood {
        Mood::Neutral => (0.92, 0.95, 0.0),
        Mood::Pleased => (0.95, 0.35, 0.0),
        Mood::Concerned => (0.7, 0.9, 0.35),
        Mood::Thinking => (0.72, 0.85, -0.15),
    };
    upper *= 1.0 - closure;
    lower *= 1.0 - closure;
    slant *= 1.0 - closure;
    let lid = Color32::from_rgb(16, 42, 60);
    let mut mesh = egui::Mesh::default();
    let mut upper_edge = Vec::new();
    let mut lower_edge = Vec::new();
    let strips = 28;
    for i in 0..strips {
        let (x0, x1) = (-r + 2.0 * r * i as f32 / strips as f32, -r + 2.0 * r * (i + 1) as f32 / strips as f32);
        for (x0, x1, top) in [(x0, x1, true), (x0, x1, false)] {
            let half = |x: f32| (r * r - x * x).max(0.0).sqrt();
            // Upper lid runs from the top of the eyeball down to its edge; a slant lowers the inner (right) side.
            let edge = |x: f32| {
                if top {
                    let y = -upper * half(x) + slant * (x + r) * 0.5;
                    y.max(-half(x)).min(half(x))
                } else {
                    (lower * half(x)).max(-half(x)).min(half(x))
                }
            };
            let (outer0, outer1) = if top { (-half(x0), -half(x1)) } else { (half(x0), half(x1)) };
            let quad = [pos2(c.x + x0, c.y + outer0), pos2(c.x + x1, c.y + outer1), pos2(c.x + x1, c.y + edge(x1)), pos2(c.x + x0, c.y + edge(x0))];
            let base = mesh.vertices.len() as u32;
            for p in quad {
                mesh.colored_vertex(p, lid);
            }
            mesh.add_triangle(base, base + 1, base + 2);
            mesh.add_triangle(base, base + 2, base + 3);
            let edge_point = pos2(c.x + x0, c.y + edge(x0));
            if top {
                upper_edge.push(edge_point)
            } else {
                lower_edge.push(edge_point)
            }
            if i == strips - 1 {
                let last = pos2(c.x + x1, c.y + edge(x1));
                if top { upper_edge.push(last) } else { lower_edge.push(last) }
            }
        }
    }
    painter.add(egui::Shape::mesh(mesh));
    painter.add(egui::Shape::line(upper_edge, Stroke::new(1.5, tint(colour, 0.9))));
    painter.add(egui::Shape::line(lower_edge, Stroke::new(1.0, tint(colour, 0.5))));
    painter.circle_stroke(c, r, Stroke::new(2.0, tint(colour, 0.9)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island;
    use physics_core::puzzle::Ch1Layout;

    /// Just after the station `next` opened: Lumen is pointing the way.
    fn walking_to(next: usize) -> Facts {
        Facts { focus_kind: Some(PuzzleKind::Rail), next: Some(next), next_station: Some(format!("Station {next}")), beckon: true, ..Facts::default() }
    }

    #[test]
    fn while_the_light_runs_the_eye_follows_it() {
        let held = Facts { held: Some("Platform: centre".into()), station_open: true, ..walking_to(1) };
        assert_eq!(choose_gaze(&held).as_deref(), Some("Platform: centre"));
        let running = Facts { light_running: true, ..held };
        assert_eq!(choose_gaze(&running).as_deref(), Some(crate::waking::LIGHT_ON_THE_ROAD));
    }

    #[test]
    fn at_the_rail_the_beam_shows_the_cargo_and_then_the_cart() {
        let open = Facts { focus_kind: Some(PuzzleKind::Rail), station_open: true, slot_empty: true, beckon: false, ..walking_to(0) };
        assert_eq!(choose_gaze(&open).as_deref(), Some(CART_BLOCK));
        let carrying = Facts { station_open: false, carrying: Some(Item::Block(Yard::Rail)), ..open.clone() };
        assert_eq!(choose_gaze(&carrying).as_deref(), Some(RAIL_CART));
        let loaded = Facts { slot_empty: false, ..open };
        assert_eq!(choose_gaze(&loaded), None, "once loaded, the beam rests unless the ruler is out");
    }

    #[test]
    fn later_stations_leave_the_screen_clear() {
        for kind in [PuzzleKind::Chase, PuzzleKind::Gravity, PuzzleKind::Channel, PuzzleKind::Ridge] {
            let base = Facts { focus_kind: Some(kind), beckon: false, ..walking_to(2) };
            assert_eq!(choose_gaze(&Facts { station_open: true, slot_empty: true, beckon: true, ..base.clone() }), None, "{kind:?}: open console");
            assert_eq!(choose_gaze(&Facts { ruler_active: true, station_open: true, ..base.clone() }), None, "{kind:?}: ruler at console");
            assert_eq!(choose_gaze(&Facts { ruler_active: true, ..base.clone() }), None, "{kind:?}: ruler while exploring");
        }
        let carrying_gate = Facts { focus_kind: Some(PuzzleKind::Gravity), carrying: Some(Item::Gate), beckon: false, ..walking_to(2) };
        assert_eq!(choose_gaze(&carrying_gate), None);
        let carrying_ledge = Facts { focus_kind: Some(PuzzleKind::Channel), carrying: Some(Item::Block(Yard::Ledge)), beckon: false, ..walking_to(3) };
        assert_eq!(choose_gaze(&carrying_ledge), None);
        // The tutor can still point on demand via [look: NAME] when a station is open.
        let asked = Facts { focus_kind: Some(PuzzleKind::Channel), station_open: true, held: Some("Platform: centre".into()), ..walking_to(3) };
        assert_eq!(choose_gaze(&asked).as_deref(), Some("Platform: centre"));
    }

    #[test]
    fn every_place_to_carry_to_is_a_landmark() {
        let layout = Ch1Layout::generate(7);
        let (_, sites) = island::build(&layout);
        let game = Game::for_test(7);
        let landmarks = Landmarks::new(&sites, &game);
        for place in [LEDGE_BLOCKS, JACK_FRAME, CART_BLOCK, RAIL_CART, LIGHT_GATE, TOWER_NOTCHES] {
            assert!(landmarks.position(place).is_some(), "{place}");
        }
        assert!(landmarks.position("Light gate").is_none(), "the gate's pin is away until it is clamped");
    }

    #[test]
    fn landmarks_follow_the_launcher_and_the_light_gate() {
        let game = Game::for_test(9);
        let (mut world, mut sites) = island::build(&game.layout);
        let mut landmarks = Landmarks::new(&sites, &game);
        let release = "Ledge launcher: release point";
        let bare = landmarks.position(release).unwrap();
        let mut dirty = crate::carry::DirtyCells::default();
        crate::carry::set_stack(&mut world, &mut sites, &mut dirty, 3);
        crate::carry::set_slot(&mut world, &mut sites, &mut dirty, crate::carry::Slot::Tower, 4);
        landmarks.follow(&sites);
        assert_eq!(landmarks.position(release), Some(bare + Vec3::Y * 3.0), "the beam points where the ruler's pin is");
        assert_eq!(landmarks.position("Light gate"), Some(sites.pins[sites.gate_pin].position));
        crate::carry::set_slot(&mut world, &mut sites, &mut dirty, crate::carry::Slot::Tower, 0);
        landmarks.follow(&sites);
        assert!(landmarks.position("Light gate").is_none(), "unclamped, the gate is not a place");
        sites.cart = sites.rail_start + Vec3::X * 5.0;
        landmarks.follow(&sites);
        assert_eq!(landmarks.position(RAIL_CART), Some(sites.cart + Vec3::Y * 0.8));
    }

    #[test]
    fn every_place_the_story_names_is_a_landmark() {
        for seed in [1, 7, 23] {
            let layout = Ch1Layout::generate(seed);
            let (_, sites) = island::build(&layout);
            let game = Game::for_test(seed);
            let landmarks = Landmarks::new(&sites, &game);
            for place in game.chapter.places_named() {
                assert!(landmarks.position(place).is_some(), "seed {seed}: {place}");
            }
            // The shore is on the island, on the side facing the main hull.
            let hull = landmarks.position(MAIN_HULL).unwrap();
            assert!(sites.shore.xz().distance(hull.xz()) < sites.wreck.xz().distance(hull.xz()), "seed {seed}");
        }
    }

    #[test]
    fn remarks_wait_for_a_quiet_moment() {
        let calm = Quiet { since_spoke: REMARK_AFTER, ..Quiet::default() };
        assert!(quiet(&calm));
        assert!(!quiet(&Quiet { since_spoke: REMARK_AFTER - 1.0, ..calm }), "not right after another line");
        for busy in [
            Quiet { station_open: true, ..calm },
            Quiet { running: true, ..calm },
            Quiet { ruler: true, ..calm },
            Quiet { conversation_open: true, ..calm },
            Quiet { carrying: true, ..calm },
            Quiet { speaking: true, ..calm },
        ] {
            assert!(!quiet(&busy), "{busy:?}");
        }
    }

    #[test]
    fn the_opening_is_not_cut_short_but_other_lines_are() {
        let mut lumen = Companion::default();
        lumen.say("A line.");
        lumen.lines[0].shown = 1.0;
        lumen.say("Something new.");
        assert_eq!(lumen.lines.len(), 2);
        assert!(lumen.lines[0].left <= 0.3, "the line on screen makes way");

        let mut lumen = Companion::default();
        lumen.say_spoken(&Spoken { text: "Once upon a time.".into(), look: None }, true);
        lumen.say_spoken(&Spoken { text: "Then this.".into(), look: None }, true);
        lumen.lines[0].shown = 1.0;
        lumen.say("Refused before launch.");
        assert_eq!(lumen.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), ["Once upon a time.", "Then this.", "Refused before launch."]);
        assert!(lumen.lines[0].left > 0.3, "the story goes on");
        // A moment later, something newer replaces what was waiting; lines of one moment stay together.
        lumen.frame += 1;
        lumen.say("Why this station.");
        lumen.say("Try the experiment.");
        assert_eq!(
            lumen.lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
            ["Once upon a time.", "Then this.", "Why this station.", "Try the experiment."]
        );
    }

    #[test]
    fn a_line_about_a_place_looks_at_it_unless_at_a_console_or_running() {
        let telling = Facts { line_look: Some(MAIN_HULL.into()), ..walking_to(0) };
        assert_eq!(choose_gaze(&telling).as_deref(), Some(MAIN_HULL));
        let at_rail = Facts { focus_kind: Some(PuzzleKind::Rail), station_open: true, slot_empty: true, ..telling.clone() };
        assert_eq!(choose_gaze(&at_rail).as_deref(), Some(CART_BLOCK));
        let at_channel = Facts { focus_kind: Some(PuzzleKind::Channel), station_open: true, ..telling.clone() };
        assert_eq!(choose_gaze(&at_channel), None);
        assert_eq!(choose_gaze(&Facts { running: true, ..telling }), None);
    }

    #[test]
    fn walking_looks_at_the_next_station_and_the_end_at_the_skiff() {
        assert_eq!(choose_gaze(&walking_to(2)).as_deref(), Some("Station 2"));
        let done = Facts { next: None, next_station: None, beckon: true, ..Facts::default() };
        assert_eq!(choose_gaze(&done).as_deref(), Some("The boat on the slipway"));
    }

    #[test]
    fn once_the_way_is_shown_the_beam_rests() {
        let f = Facts { beckon: false, ..walking_to(2) };
        assert_eq!(choose_gaze(&f), None);
        // Standing at a finished (or locked) station, the way on is shown again.
        let at_done = Facts { at_finished_station: true, ..f };
        assert_eq!(choose_gaze(&at_done).as_deref(), Some("Station 2"));
    }

    #[test]
    fn nothing_about_a_solved_puzzle_draws_the_beam() {
        let solved = Facts { focus_kind: Some(PuzzleKind::Rail), focus_solved: true, beckon: false, ..walking_to(1) };
        assert_eq!(choose_gaze(&Facts { station_open: true, slot_empty: true, ..solved.clone() }), None);
        assert_eq!(choose_gaze(&Facts { ruler_active: true, ..solved.clone() }), None);
        assert_eq!(choose_gaze(&Facts { running: true, ..solved }), None);
    }

    #[test]
    fn the_ruler_is_led_between_pins_at_the_first_station_only() {
        let mut f = Facts { ruler_active: true, station_open: true, beckon: false, ..walking_to(0) };
        assert_eq!(choose_gaze(&f).as_deref(), Some("Rail cart: starting point"));
        f.ruler_first = Some("Rail cart: starting point".into());
        assert_eq!(choose_gaze(&f).as_deref(), Some("Rail marker: centre"));
        // Measured the other way round: point back at the start.
        f.ruler_first = Some("Rail marker: centre".into());
        assert_eq!(choose_gaze(&f).as_deref(), Some("Rail cart: starting point"));
        // At a later station, the player chooses what to measure without a beam.
        f.focus_kind = Some(PuzzleKind::Chase);
        assert_eq!(choose_gaze(&f), None);
    }

    #[test]
    fn running_experiments_and_tests_keep_the_beam_off() {
        let mut f = Facts { ruler_active: true, held: Some("The wreck".into()), ..walking_to(0) };
        assert_eq!(choose_gaze(&f).as_deref(), Some("The wreck"));
        f.running = true;
        assert_eq!(choose_gaze(&f), None);
        // The waking light still takes priority over everything else.
        f.light_running = true;
        assert_eq!(choose_gaze(&f).as_deref(), Some(crate::waking::LIGHT_ON_THE_ROAD));
    }

    #[test]
    fn first_sentences() {
        assert_eq!(first_sentence("They land together. Firing it sideways adds nothing."), "They land together.");
        assert_eq!(first_sentence("One sentence."), "One sentence.");
    }

    #[test]
    fn every_guided_pin_is_on_the_island() {
        let layout = Ch1Layout::generate(7);
        let (_, sites) = island::build(&layout);
        let labels: Vec<&str> = sites.pins.iter().map(|p| p.label).collect();
        for label in RAIL_MEASURE {
            assert!(labels.contains(&label), "no pin labelled '{label}'");
        }
    }

    #[test]
    fn edge_points_lie_on_the_edge() {
        let rect = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 100.0));
        assert_eq!(edge_point(rect, vec2(1.0, 0.0)), pos2(200.0, 50.0));
        assert_eq!(edge_point(rect, vec2(0.0, -1.0)), pos2(100.0, 0.0));
        let corner = edge_point(rect, vec2(1.0, 1.0).normalized());
        assert!((corner.y - 100.0).abs() < 1e-3 && (corner.x - 150.0).abs() < 1e-3);
    }

    #[test]
    fn the_eye_comes_forward_to_the_middle_of_what_is_free() {
        let screen = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 720.0));
        let corner = eye_place(screen, 0.0, 0.0, screen.right());
        assert_eq!(corner, pos2(EYE_INSET.x, 720.0 - EYE_INSET.y));
        let middle = eye_place(screen, 0.0, 1.0, screen.right());
        assert_eq!(middle.x, 640.0);
        assert!(middle.y < corner.y, "it floats a little higher, over the hints");
        // With a station open, it centres on what the panel leaves.
        let beside = eye_place(screen, 0.0, 1.0, screen.right() - STATION_PANEL);
        assert_eq!(beside.x, (1280.0 - STATION_PANEL) / 2.0);
        assert!(beside.x + EYE_REACH < screen.right() - STATION_PANEL);
    }

    #[test]
    fn the_eye_stays_above_the_letterbox() {
        for (w, h) in [(1280.0, 720.0), (1920.0, 1080.0), (2560.0, 1440.0)] {
            let screen = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h));
            let bar = crate::cinematic::letterbox_height(screen, 1.0);
            for forward in [0.0, 1.0] {
                let eye = eye_place(screen, lift_for(bar, false), forward, screen.right());
                assert!(
                    eye.y + EYE_REACH <= screen.bottom() - bar,
                    "{w}x{h}: the eye reaches {} below the bar's top",
                    eye.y + EYE_REACH - (screen.bottom() - bar)
                );
            }
        }
        // With no letterbox the eye stays where it was; the ruler still lifts it.
        assert_eq!(lift_for(0.0, false), 0.0);
        assert_eq!(lift_for(0.0, true), RULER_LIFT);
    }

    #[test]
    fn a_bubble_stays_on_screen_over_its_eye() {
        let screen = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 720.0));
        let size = vec2(300.0, 60.0);
        let middle = eye_place(screen, 0.0, 1.0, screen.right());
        let rect = bubble_rect(screen, middle, size, 1.0);
        assert!((rect.center().x - middle.x).abs() < 1e-3 && rect.bottom() < middle.y - EYE);
        let corner = eye_place(screen, 0.0, 0.0, screen.right());
        let rect = bubble_rect(screen, corner, size, 1.0);
        assert!(rect.left() >= 8.0, "centred over the corner it would run off the screen");
    }

    #[test]
    fn spoken_lines_can_be_advanced_and_are_kept_in_history() {
        let mut companion = Companion::default();
        companion.say("First line.");
        companion.say("Second line.");
        assert_eq!(companion.history, ["First line.", "Second line."]);
        assert!(companion.advance_line());
        assert_eq!(companion.lines.len(), 1);
        assert_eq!(companion.lines[0].text, "Second line.");
        assert!(companion.advance_line());
        assert!(companion.lines.is_empty());
        assert!(!companion.advance_line());
        // Consecutive duplicates are not logged twice, and history is capped.
        companion.say("Second line.");
        assert_eq!(companion.history.len(), 2);
        for i in 0..50 {
            companion.say(format!("Line {i}."));
        }
        assert_eq!(companion.history.len(), MAX_HISTORY);
        assert_eq!(companion.history.last().unwrap(), "Line 49.");
    }

    #[test]
    fn landmark_labels_move_clear_of_open_panels() {
        let screen = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(1280.0, 720.0));
        let dock = egui::Rect::from_min_max(pos2(10.0, 10.0), pos2(270.0, 150.0));
        let placed = place_label(screen, pos2(180.0, 155.0), vec2(140.0, 22.0), None, &[dock]);
        assert!(!placed.intersects(dock), "label {placed:?} should not overlap dock {dock:?}");
        assert!(screen.contains_rect(placed));
    }

    #[test]
    fn station_dialogue_presents_experiment_topics_and_hides_panel_on_first_open() {
        let chapter = physics_core::content::chapter_1_1();
        let channel = chapter.puzzle("1.1.3").unwrap();
        let demo = channel.demo.clone().unwrap();
        let mut companion = Companion::default();

        // First open plays all presentation beats, hides the console panel, and points at landmarks.
        companion.open_dialogue(Console::Station(3), &channel.dialogue, true, true, Some(NextBeat::Experiment(3, demo)));
        assert!(companion.waiting_line());
        assert!(companion.hiding_console_panel());
        assert!(matches!(&companion.dialogue.as_ref().unwrap().stage, DialogueStage::Lines(lines) if lines.len() == channel.dialogue.present.len()));
        let first_look = companion.dialogue.as_ref().unwrap().look().map(str::to_string);
        assert!(first_look.is_some());
        let gaze = choose_gaze(&Facts { focus_kind: Some(PuzzleKind::Channel), station_open: true, dialogue_look: first_look.clone(), ..walking_to(3) });
        assert_eq!(gaze, first_look, "station dialogue points at what it names");

        // Advance to the final presentation beat: still has_more because the experiment follows.
        assert!(companion.advance_line());
        assert!(companion.dialogue.as_ref().unwrap().has_more());
        assert!(companion.hiding_console_panel());

        // Advancing from the final beat enters the "Try it first" experiment prompt.
        assert!(companion.advance_line());
        assert!(matches!(&companion.dialogue.as_ref().unwrap().stage, DialogueStage::Experiment { puzzle: 3, .. }));
        assert!(!companion.dialogue.as_ref().unwrap().has_more());
        assert!(companion.hiding_console_panel());

        // After the experiment runs, Reveal shows with topic chips and still hides the panel until dismissed.
        companion.open_demo_reveal(
            3,
            &channel.dialogue,
            Some(("You called it!".into(), true)),
            channel.demo.as_ref().unwrap().reveal.clone(),
            vec![("independence".into(), "Horizontal and vertical motion are independent".into())],
            true,
        );
        assert!(companion.hiding_console_panel());
        assert!(companion.pick_topic(0));
        assert!(companion.asked.contains(&(Console::Station(3), 0)));
        assert!(companion.hiding_console_panel(), "panel stays hidden while reading initial Q&A topics");
        assert!(matches!(&companion.dialogue.as_ref().unwrap().stage, DialogueStage::Lines(lines) if lines.len() == channel.dialogue.topics[0].lines.len()));

        // Advancing through the topic reply dismisses the dialogue and reveals the console panel.
        for _ in 0..channel.dialogue.topics[0].lines.len() {
            assert!(companion.advance_line());
        }
        assert!(companion.dialogue.is_none());
        assert!(!companion.hiding_console_panel());

        // Re-opening via Briefing jumps directly to the last presentation line and keeps the panel visible.
        companion.open_dialogue(Console::Station(3), &channel.dialogue, false, false, None);
        assert!(!companion.hiding_console_panel());
        assert!(!companion.dialogue.as_ref().unwrap().has_more());
        assert!(matches!(&companion.dialogue.as_ref().unwrap().stage, DialogueStage::Lines(lines) if lines.len() == 1));
    }

    #[test]
    fn auto_briefing_is_skipped_on_reloaded_progress() {
        let mut game = Game::for_test(7);
        assert!(should_auto_present_station(&game, 0));
        assert!(should_auto_present_station(&game, 1));
        assert!(should_auto_present_beam(&game));

        // Watching a station's experiment skips its automatic briefing on reload.
        let chase_id = game.def(1).id.clone();
        game.save.record_demo(&chase_id, Some(0));
        assert!(!should_auto_present_station(&game, 1));

        // Making an attempt at a station without a demo skips its automatic briefing too.
        let rail_id = game.def(0).id.clone();
        game.save.puzzles.entry(rail_id).or_default().attempts.push(physics_core::progress::Attempt {
            values: Vec::new(),
            pass: false,
            headline: "Short".into(),
            detail: String::new(),
        });
        assert!(!should_auto_present_station(&game, 0));

        // Holding a load on the beam skips the beam's automatic briefing.
        game.save.sides.entry(crate::beam::BEAM_ID.into()).or_default().held = 1;
        assert!(!should_auto_present_beam(&game));
    }

    #[test]
    fn story_lines_wait_instead_of_fading_out() {
        let mut companion = Companion::default();
        companion.say_spoken(&Spoken { text: "Opening line.".into(), look: None }, true);
        assert!(companion.telling());
        assert!(companion.waiting_line());
        assert!(companion.hiding_console_panel(), "console stays hidden while Lumen is still telling a story");
        assert!(companion.lines[0].wait);
        assert!(companion.advance_line());
        assert!(!companion.hiding_console_panel());
        // Remarks still auto-fade and do not hide the console.
        let mut remarking = Companion::default();
        assert!(remarking.remark(&Spoken { text: "Quiet remark.".into(), look: None }));
        assert!(!remarking.waiting_line());
        assert!(!remarking.hiding_console_panel());
    }

    #[test]
    fn station_zero_measure_guide_advances_and_recovers_on_cancel() {
        let chapter = physics_core::content::chapter_1_1();
        let rail = chapter.puzzle("1.1.0").unwrap();
        let mut companion = Companion::default();
        let mut ruler = Ruler::default();

        // First open at Station 0 leads from the presentation beats into the action-gated guide.
        companion.open_dialogue(Console::Station(0), &rail.dialogue, true, true, Some(NextBeat::MeasureGuide));
        assert!(companion.advance_line());
        assert!(companion.dialogue.as_ref().unwrap().has_more());
        assert!(companion.advance_line());
        assert!(matches!(&companion.dialogue.as_ref().unwrap().stage, DialogueStage::Guide(GuideStep::PressR)));
        assert!(!companion.dialogue.as_ref().unwrap().has_more(), "Enter / Next cannot skip an action-gated guide step");
        assert!(companion.hiding_console_panel());

        // Pressing R moves to PickFirstPin; cancelling steps smoothly back to PressR.
        ruler.start();
        companion.sync_measure_guide(&ruler, false, false);
        assert!(matches!(&companion.dialogue.as_ref().unwrap().stage, DialogueStage::Guide(GuideStep::PickFirstPin)));
        ruler.put_away();
        companion.sync_measure_guide(&ruler, false, false);
        assert!(matches!(&companion.dialogue.as_ref().unwrap().stage, DialogueStage::Guide(GuideStep::PressR)));

        // Full sequence: R -> first pin -> second pin -> note reading -> open notebook -> wrap-up.
        let picked = |x: f32| crate::ruler::Picked { point: Vec3::new(x, 0.0, 0.0), label: String::new(), snapped: true };
        ruler.start();
        companion.sync_measure_guide(&ruler, false, false);
        ruler.pick(picked(0.0));
        companion.sync_measure_guide(&ruler, false, false);
        assert!(matches!(&companion.dialogue.as_ref().unwrap().stage, DialogueStage::Guide(GuideStep::PickSecondPin)));
        ruler.pick(picked(10.0));
        companion.sync_measure_guide(&ruler, false, false);
        assert!(matches!(&companion.dialogue.as_ref().unwrap().stage, DialogueStage::Guide(GuideStep::NoteReading)));

        // Noting the reading asks the player to open the notebook.
        companion.sync_measure_guide(&ruler, true, false);
        assert!(matches!(&companion.dialogue.as_ref().unwrap().stage, DialogueStage::Guide(GuideStep::OpenNotebook)));

        // Opening the notebook finishes the guide and transitions to the wrap-up beat with topics.
        companion.sync_measure_guide(&ruler, true, true);
        assert!(matches!(&companion.dialogue.as_ref().unwrap().stage, DialogueStage::Lines(lines) if lines[0].text == GUIDE_DONE_LINE));
        assert!(companion.hiding_console_panel(), "panel stays hidden until the player clicks Open console");
    }

    #[test]
    fn awaited_experiment_is_forgotten_when_dropped_or_ended() {
        let waiting = |hide_panel| AwaitingDemo { puzzle: 3, hide_panel, first_run: true, started: false };
        // Queued, not yet taken by the station: still waiting, and the console stays hidden.
        let mut companion = Companion { awaiting_demo: Some(waiting(true)), ..Companion::default() };
        companion.track_awaiting_demo(true, None);
        assert!(companion.awaiting_demo.is_some());
        assert!(companion.hiding_console_panel());

        // The station took the request and dropped it without running anything: forgotten, so the
        // console comes back.
        companion.track_awaiting_demo(false, None);
        assert!(companion.awaiting_demo.is_none());
        assert!(!companion.hiding_console_panel());

        // Another station's run doesn't count as this one starting.
        companion.awaiting_demo = Some(waiting(true));
        companion.track_awaiting_demo(false, Some(2));
        assert!(companion.awaiting_demo.is_none());

        // A run that starts is followed until it ends; anything left over then is forgotten.
        companion.awaiting_demo = Some(waiting(true));
        companion.track_awaiting_demo(false, Some(3));
        assert!(companion.awaiting_demo.is_some_and(|a| a.started));
        companion.track_awaiting_demo(false, Some(3));
        assert!(companion.awaiting_demo.is_some());
        companion.track_awaiting_demo(false, None);
        assert!(companion.awaiting_demo.is_none());
    }

    #[test]
    fn first_pages_in_a_reveal_wait_for_the_handbook() {
        let chapter = physics_core::content::chapter_1_1();
        let channel = chapter.puzzle("1.1.3").unwrap();
        let reveal = channel.demo.as_ref().unwrap().reveal.clone();
        let pages = vec![("independence".to_string(), "Horizontal and vertical motion are independent".to_string())];
        let mut companion = Companion::default();

        // The player's first pages: the reveal waits for the handbook, the console stays hidden.
        companion.open_demo_reveal(3, &channel.dialogue, None, reveal.clone(), pages.clone(), true);
        assert!(companion.reveal_waits_for_handbook());
        assert!(companion.hiding_console_panel());

        // Closed early (the player walked off, say): the first-time handbook prompt takes over.
        companion.hand_over_handbook_gate();
        companion.dialogue = None;
        assert!(companion.handbook_prompt.as_ref().is_some_and(|p| p.text == first_page_prompt(&pages[0].1)));

        // Once the handbook has been opened, a reveal with new pages offers its chips at once.
        companion.handbook_prompt = None;
        companion.handbook_opened = true;
        companion.open_demo_reveal(3, &channel.dialogue, None, reveal.clone(), pages, true);
        assert!(!companion.reveal_waits_for_handbook());

        // A reveal with no new pages never waits.
        companion.handbook_opened = false;
        companion.open_demo_reveal(3, &channel.dialogue, None, reveal, Vec::new(), true);
        assert!(!companion.reveal_waits_for_handbook());
    }
}
