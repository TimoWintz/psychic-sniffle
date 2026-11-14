//! Lumen's on-screen body: a holographic eye at the lower left that looks at things in the world,
//! points at them with a beam of light, and says short lines.
//!
//! What it looks at comes from the state of the game (the director below), or for a while from a
//! `[look: NAME]` tag in a tutor reply. Either way it is only a gaze: nothing here changes the game.
//! The scripted lines never contain a number, so they cannot give an answer away.

use crate::island::Sites;
use crate::ruler::Ruler;
use crate::sim::Sim;
use crate::state::{Game, PUZZLES, Ui};
use crate::tutor_bridge::Lumen;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use egui::{Color32, Pos2, Stroke, Vec2 as EVec2, pos2, vec2};
use physics_core::puzzle::PuzzleKind;

/// How long Lumen keeps looking where the tutor pointed, in seconds.
const MODEL_LOOK_SECONDS: f32 = 9.0;
/// How long Lumen keeps looking at the target after a test.
const RESULT_LOOK_SECONDS: f32 = 6.0;

/// Places Lumen can look at, by name, with where they are.
#[derive(Resource, Clone, Debug)]
pub struct Landmarks(Vec<(String, Vec3)>);

impl Landmarks {
    pub fn new(sites: &Sites, game: &Game) -> Self {
        let mut places: Vec<(String, Vec3)> = sites.pins.iter().map(|pin| (pin.label.to_string(), pin.position)).collect();
        for (i, console) in sites.stations.iter().enumerate() {
            // The station's screen, under its column of light.
            places.push((station_name(game, i), console.as_vec3() + Vec3::new(0.5, 1.3, 0.5)));
        }
        places.push(("The wreck".into(), sites.wreck + Vec3::Y * 2.0));
        places.push(("The shipyard floor".into(), sites.shipyard + Vec3::Y * 1.0));
        Landmarks(places)
    }

    pub fn names(&self) -> Vec<String> {
        self.0.iter().map(|(name, _)| name.clone()).collect()
    }

    pub fn position(&self, name: &str) -> Option<Vec3> {
        self.0.iter().find(|(n, _)| n == name).map(|(_, p)| *p)
    }
}

pub fn station_name(game: &Game, puzzle: usize) -> String {
    format!("Station: {}", game.def(puzzle).title)
}

/// The survey pins that matter for each kind of puzzle, by their labels on the island.
struct Guide {
    /// What the ruler should be taken between, in order.
    measure: [&'static str; 2],
    /// Where to look while the station is open.
    study: &'static str,
    /// Where the test is judged.
    target: &'static str,
}

fn guide(kind: PuzzleKind) -> Guide {
    match kind {
        PuzzleKind::Rail => Guide {
            measure: ["Rail cart: starting point", "Rail marker: centre"],
            study: "Rail marker: centre",
            target: "Rail marker: centre",
        },
        PuzzleKind::Gravity => Guide {
            measure: ["Drop tower: release point", "Drop tower: landing plate"],
            study: "Drop tower: release point",
            target: "Survey mast: landing plate",
        },
        PuzzleKind::Canyon => Guide {
            measure: ["Ledge launcher: release point", "Platform: centre"],
            study: "Platform: centre",
            target: "Platform: centre",
        },
        PuzzleKind::Ridge => Guide {
            measure: ["Tilting launcher: release point", "Shipyard pad: centre"],
            study: "Ridge: top, near edge",
            target: "Shipyard pad: centre",
        },
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mood {
    #[default]
    Neutral,
    Pleased,
    Concerned,
    Thinking,
}

#[derive(Clone, Debug)]
struct Line {
    text: String,
    left: f32,
    shown: f32,
}

/// Lumen's state: where it looks, how it feels, what it is saying, and its animation.
#[derive(Resource, Default)]
pub struct Companion {
    /// Name of the place Lumen is looking at, if any (one of the [`Landmarks`]).
    pub gaze: Option<String>,
    pub mood: Mood,
    /// A gaze that outranks the scripted one for a while: (place, seconds left, from the tutor).
    held: Option<(String, f32, bool)>,
    mood_left: f32,
    lines: Vec<Line>,
    anim: Animation,
}

impl Companion {
    /// Say a line. Something new to say cuts short what is being said and drops what was waiting,
    /// so the line on screen is always about what just happened. Lines said in the same moment are
    /// kept, in order.
    fn say(&mut self, text: impl Into<String>) {
        let text = text.into();
        if let Some(current) = self.lines.first_mut()
            && current.shown > 0.0
        {
            current.left = current.left.min(0.3);
            self.lines.truncate(1);
        }
        let words = text.split_whitespace().count() as f32;
        // Long enough to read, then out of the way.
        self.lines.push(Line { text, left: (2.5 + words * 0.32).min(10.0), shown: 0.0 });
    }

    fn feel(&mut self, mood: Mood, seconds: f32) {
        self.mood = mood;
        self.mood_left = seconds;
    }

    fn hold(&mut self, place: &str, seconds: f32, from_tutor: bool) {
        self.held = Some((place.to_string(), seconds, from_tutor));
    }
}

/// What the director looks at to decide where Lumen's eye goes. A plain copy, so the decision can
/// be tested without the engine.
#[derive(Clone, Debug, Default)]
struct Facts {
    /// Kind of the puzzle the player is working on.
    focus_kind: Option<PuzzleKind>,
    station_open: bool,
    focus_solved: bool,
    /// The next puzzle not yet solved, if any.
    next: Option<usize>,
    next_station: Option<String>,
    /// A test of this kind of puzzle is running.
    testing: Option<PuzzleKind>,
    /// The free drop at the tower is running.
    tower_drop: bool,
    ruler_active: bool,
    /// Label of the ruler's first point, while it waits for the second.
    ruler_first: Option<String>,
    ruler_done: bool,
    /// A place that outranks the rest for now.
    held: Option<String>,
}

/// Where Lumen should look, in priority order.
fn choose_gaze(f: &Facts) -> Option<String> {
    if let Some(held) = &f.held {
        return Some(held.clone());
    }
    if let Some(kind) = f.testing {
        return Some(guide(kind).target.into());
    }
    if f.tower_drop {
        return Some("Drop tower: landing plate".into());
    }
    if f.ruler_active
        && let Some(kind) = f.focus_kind
    {
        let [a, b] = guide(kind).measure;
        return match (&f.ruler_first, f.ruler_done) {
            (None, false) => Some(a.into()),
            // Whichever of the two has not been clicked yet.
            (Some(first), false) => Some(if first == b { a } else { b }.into()),
            (_, true) => Some(b.into()),
        };
    }
    if f.station_open
        && let Some(kind) = f.focus_kind
    {
        return Some(if f.focus_solved { guide(kind).target } else { guide(kind).study }.into());
    }
    match &f.next_station {
        Some(station) => Some(station.clone()),
        None if f.next.is_none() => Some("The shipyard floor".into()),
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
}

pub struct CompanionPlugin;

impl Plugin for CompanionPlugin {
    fn build(&self, app: &mut App) {
        let landmarks = Landmarks::new(app.world().resource::<Sites>(), app.world().resource::<Game>());
        app.insert_resource(landmarks)
            .init_resource::<Companion>()
            .add_systems(Update, direct)
            .add_systems(EguiPrimaryContextPass, draw.after(crate::ui::draw));
    }
}

#[allow(clippy::too_many_arguments)]
fn direct(
    time: Res<Time>,
    game: Res<Game>,
    ui: Res<Ui>,
    sim: Res<Sim>,
    ruler: Res<Ruler>,
    landmarks: Res<Landmarks>,
    mut lumen: ResMut<Lumen>,
    mut companion: ResMut<Companion>,
    mut last: Local<Option<Seen>>,
) {
    let dt = time.delta_secs();
    let companion = &mut *companion;
    let focus = ui.focus(&game);
    let focus_kind = game.def(focus).kind;
    let next = game.next_unsolved();

    // The tutor pointed somewhere.
    for conversation in &mut lumen.conversations {
        if let Some(place) = conversation.take_look()
            && landmarks.position(&place).is_some()
        {
            companion.hold(&place, MODEL_LOOK_SECONDS, true);
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
    };
    match last.as_ref() {
        None => {
            // The first frame: a greeting.
            let fresh = game.save.puzzles.values().all(|p| p.attempts.is_empty());
            match next {
                Some(n) if fresh => {
                    let intro = &game.chapter.intro;
                    let first_sentence = intro.split_inclusive(". ").next().unwrap_or(intro).trim();
                    companion.say(first_sentence);
                    companion.say(format!("{} Follow my beam to the station.", game.def(n).task));
                }
                Some(n) => companion.say(format!("Welcome back. Next: {}", lowercase_first(&game.def(n).task))),
                None => companion.say("Welcome back. The frame stands on the shipyard floor."),
            }
        }
        Some(before) => {
            for puzzle in 0..PUZZLES {
                if now.results[puzzle] && !before.results[puzzle] {
                    react_to_result(companion, &game, &ui, puzzle);
                }
            }
            if now.refusal.is_some() && now.refusal != before.refusal {
                companion.feel(Mood::Concerned, 4.0);
                companion.say("Refused before launch, so no charge was used. The reason is under the button.");
            }
            if now.next != before.next {
                match now.next {
                    Some(n) => companion.say(format!("Next: {} Follow my beam.", lowercase_first(&game.def(n).task))),
                    None => companion.say("That is the whole frame. Come and see it on the shipyard floor."),
                }
            }
            if now.ruler_active && !before.ruler_active {
                companion.say("Click the yellow pin I am pointing at.");
            } else if now.ruler_active && now.ruler_first && !before.ruler_first && ruler.second.is_none() {
                companion.say("Now this one.");
            }
            if now.asking && !before.asking {
                companion.feel(Mood::Thinking, f32::INFINITY);
            } else if !now.asking && before.asking && companion.mood == Mood::Thinking {
                companion.feel(Mood::Neutral, 0.0);
            }
        }
    }
    *last = Some(now);

    // Time passes for held looks, moods and lines.
    if let Some((_, left, _)) = &mut companion.held {
        *left -= dt;
        if *left <= 0.0 {
            companion.held = None;
        }
    }
    companion.mood_left -= dt;
    if companion.mood_left <= 0.0 && companion.mood != Mood::Neutral {
        companion.mood = Mood::Neutral;
    }
    if let Some(line) = companion.lines.first_mut() {
        line.shown += dt;
        line.left -= dt;
        if line.left <= 0.0 {
            companion.lines.remove(0);
        }
    }

    let testing = sim.testing().map(|p| game.def(p).kind);
    if testing.is_some() && !asking {
        companion.feel(Mood::Thinking, 0.3);
    }
    let facts = Facts {
        focus_kind: Some(focus_kind),
        station_open: ui.station.is_some(),
        focus_solved: game.solved(focus),
        next,
        next_station: next.map(|n| station_name(&game, n)),
        testing,
        tower_drop: sim.busy() && testing.is_none(),
        ruler_active: ruler.active,
        ruler_first: ruler.first.as_ref().filter(|_| ruler.second.is_none()).map(|p| p.label.clone()),
        ruler_done: ruler.second.is_some(),
        held: companion.held.as_ref().map(|(place, _, _)| place.clone()),
    };
    companion.gaze = choose_gaze(&facts).filter(|place| landmarks.position(place).is_some());
}

fn react_to_result(companion: &mut Companion, game: &Game, ui: &Ui, puzzle: usize) {
    let Some(outcome) = &ui.results[puzzle] else { return };
    let kind = game.def(puzzle).kind;
    let target = guide(kind).target;
    companion.hold(target, RESULT_LOOK_SECONDS, false);
    if outcome.pass {
        companion.feel(Mood::Pleased, 5.0);
        let cheers = ["Right on the mark.", "The world agrees with you.", "Clean. That one is ours.", "Over and down, right where you said."];
        companion.say(cheers[puzzle % cheers.len()]);
    } else {
        companion.feel(Mood::Concerned, 5.0);
        companion.say(match kind {
            PuzzleKind::Gravity => "Not quite. Compare your prediction with the real fall.",
            _ => "Not this time. Look where it ended, compared with the target.",
        });
    }
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

#[derive(Default)]
struct Animation {
    clock: f32,
    next_blink: f32,
    blink: f32,
    /// Smoothed offset of the iris from the eye's centre, as a fraction of the eye.
    iris: EVec2,
    /// How far the eye has risen, in points.
    lift: f32,
    /// Smoothed end of the beam on screen, and how visible the beam is.
    beam_end: Option<Pos2>,
    beam: f32,
    colour: [f32; 3],
}

fn eye_centre(ctx: &egui::Context, clock: f32, lift: f32) -> Pos2 {
    let screen = ctx.content_rect();
    pos2(screen.left() + EYE_INSET.x, screen.bottom() - EYE_INSET.y - lift + (clock * 1.7).sin() * 3.0)
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

fn draw(
    mut contexts: EguiContexts,
    time: Res<Time>,
    landmarks: Res<Landmarks>,
    mut companion: ResMut<Companion>,
    mut ui_state: ResMut<Ui>,
    ruler: Res<Ruler>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let dt = time.delta_secs().min(0.1);
    let companion = &mut *companion;
    let mood = companion.mood;
    let gaze = companion.gaze.clone();
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
    let lift = if ruler.active { RULER_LIFT } else { 0.0 };
    a.lift += (lift - a.lift) * (dt * 6.0).min(1.0);
    let centre = eye_centre(&ctx, t, a.lift);
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
            let toward_middle = d.map_or(vec2(0.0, -26.0), |d| -d * 34.0);
            let mut rect = egui::Rect::from_center_size(end + toward_middle, size);
            rect = rect.translate(vec2(
                (screen.left() + 4.0 - rect.left()).max(0.0) - (rect.right() - screen.right() + 4.0).max(0.0),
                (screen.top() + 4.0 - rect.top()).max(0.0) - (rect.bottom() - screen.bottom() + 4.0).max(0.0),
            ));
            painter.rect_filled(rect, 4.0, Color32::from_rgba_unmultiplied(12, 22, 34, (200.0 * a.beam) as u8));
            painter.rect_stroke(rect, 4.0, Stroke::new(1.0, tint(colour, 0.6 * a.beam)), egui::StrokeKind::Inside);
            painter.galley(rect.min + vec2(6.0, 3.0), galley, Color32::WHITE);
        }
    }

    // The eye itself, which can be clicked to talk.
    let reach = EYE + 16.0;
    let area = egui::Area::new(egui::Id::new("lumen-eye"))
        .order(egui::Order::Foreground)
        .fixed_pos(centre - vec2(reach, reach))
        .show(&ctx, |ui| {
            let (rect, response) = ui.allocate_exact_size(vec2(reach, reach) * 2.0, egui::Sense::click());
            paint_eye(ui.painter(), rect.center(), colour, mood, closure, a.iris, t, response.hovered());
            response.on_hover_text("Lumen. Click or press T to talk.")
        });
    if area.inner.clicked() {
        ui_state.show_tutor = !ui_state.show_tutor;
    }

    // What Lumen is saying, unless the conversation window is open.
    if !ui_state.show_tutor
        && let Some(line) = companion.lines.first()
    {
        let alpha = (line.shown / 0.25).min(1.0) * (line.left / 0.5).min(1.0);
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("lumen-says")));
        let galley = painter.layout(line.text.clone(), egui::FontId::proportional(14.0), tint([235.0, 245.0, 255.0], alpha), 250.0);
        let size = galley.size() + vec2(20.0, 14.0);
        let anchor = centre + vec2(-EYE + 2.0, -EYE - 22.0);
        let rect = egui::Rect::from_min_size(anchor - vec2(0.0, size.y), size);
        painter.rect_filled(rect, 8.0, Color32::from_rgba_unmultiplied(12, 22, 34, (225.0 * alpha) as u8));
        painter.rect_stroke(rect, 8.0, Stroke::new(1.5, tint(colour, 0.8 * alpha)), egui::StrokeKind::Inside);
        let tail = [pos2(rect.left() + 18.0, rect.bottom()), pos2(rect.left() + 34.0, rect.bottom()), pos2(centre.x - 4.0, centre.y - EYE - 6.0)];
        painter.add(egui::Shape::convex_polygon(tail.to_vec(), Color32::from_rgba_unmultiplied(12, 22, 34, (225.0 * alpha) as u8), Stroke::NONE));
        painter.line_segment([tail[0], tail[2]], Stroke::new(1.5, tint(colour, 0.8 * alpha)));
        painter.line_segment([tail[1], tail[2]], Stroke::new(1.5, tint(colour, 0.8 * alpha)));
        painter.galley(rect.min + vec2(10.0, 7.0), galley, Color32::WHITE);
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
            if top { upper_edge.push(edge_point) } else { lower_edge.push(edge_point) }
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

    fn walking_to(next: usize) -> Facts {
        Facts { focus_kind: Some(PuzzleKind::Rail), next: Some(next), next_station: Some(format!("Station {next}")), ..Facts::default() }
    }

    #[test]
    fn walking_looks_at_the_next_station_and_the_end_at_the_shipyard() {
        assert_eq!(choose_gaze(&walking_to(2)).as_deref(), Some("Station 2"));
        let done = Facts { next: None, next_station: None, ..Facts::default() };
        assert_eq!(choose_gaze(&done).as_deref(), Some("The shipyard floor"));
    }

    #[test]
    fn the_ruler_is_led_from_one_pin_to_the_other() {
        let mut f = Facts { ruler_active: true, station_open: true, ..walking_to(0) };
        assert_eq!(choose_gaze(&f).as_deref(), Some("Rail cart: starting point"));
        f.ruler_first = Some("Rail cart: starting point".into());
        assert_eq!(choose_gaze(&f).as_deref(), Some("Rail marker: centre"));
        // Measured the other way round: point back at the start.
        f.ruler_first = Some("Rail marker: centre".into());
        assert_eq!(choose_gaze(&f).as_deref(), Some("Rail cart: starting point"));
    }

    #[test]
    fn a_running_test_and_a_held_look_come_first() {
        let mut f = Facts { ruler_active: true, testing: Some(PuzzleKind::Canyon), ..walking_to(2) };
        assert_eq!(choose_gaze(&f).as_deref(), Some("Platform: centre"));
        f.held = Some("The wreck".into());
        assert_eq!(choose_gaze(&f).as_deref(), Some("The wreck"));
    }

    #[test]
    fn every_guided_pin_is_on_the_island() {
        let layout = Ch1Layout::generate(7);
        let (_, sites) = island::build(&layout);
        let labels: Vec<&str> = sites.pins.iter().map(|p| p.label).collect();
        for kind in [PuzzleKind::Rail, PuzzleKind::Gravity, PuzzleKind::Canyon, PuzzleKind::Ridge] {
            let g = guide(kind);
            for label in g.measure.iter().chain([&g.study, &g.target]) {
                assert!(labels.contains(label), "{kind:?}: no pin labelled '{label}'");
            }
        }
        assert!(labels.contains(&"Drop tower: landing plate"));
    }

    #[test]
    fn edge_points_lie_on_the_edge() {
        let rect = egui::Rect::from_min_size(pos2(0.0, 0.0), vec2(200.0, 100.0));
        assert_eq!(edge_point(rect, vec2(1.0, 0.0)), pos2(200.0, 50.0));
        assert_eq!(edge_point(rect, vec2(0.0, -1.0)), pos2(100.0, 0.0));
        let corner = edge_point(rect, vec2(1.0, 1.0).normalized());
        assert!((corner.y - 100.0).abs() < 1e-3 && (corner.x - 150.0).abs() < 1e-3);
    }
}
