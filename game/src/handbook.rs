//! The handbook's pages: their text, a moving picture of the idea in a scene from Earth, and an
//! optional quiz.
//!
//! The pictures are drawn like the station's result graphs, with flat shapes painted in egui. They
//! show Earth on purpose: the castaway's home, never the island's own setups, so a picture explains
//! without answering a puzzle. Their motion is worked out in closed form by [`tracks`], which is pure
//! and tested; drawing only turns it into shapes.

use crate::state::{Anim, Game, Ui};
use crate::ui::{BAD, DIM, GOOD};
use bevy_egui::egui;
use egui::{Align2, Color32, FontId, Pos2, RichText, Stroke, Vec2};
use physics_core::ballistics::Projectile;
use physics_core::content::{Concept, Illustration, Scene};
use physics_core::materials::Material;
use physics_core::units::sig;

/// Earth's gravitational acceleration, for the pictures and the quizzes. The island's is never near
/// it (see `Ch1Layout::G_EXCLUDED`).
const EARTH_G: f64 = 9.81;
/// The widest a picture is drawn, in interface points.
const PICTURE_WIDTH: f32 = 470.0;
/// How long a finished scene holds still before it plays again, in seconds of the clock on the wall.
const HOLD: f64 = 1.4;

/// One moving thing in a scene: where it is now, and where it was at each flash so far.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub now: (f64, f64),
    pub ghosts: Vec<(f64, f64)>,
}

/// How a scene plays: how long its motion lasts, how much slower than life it is shown, and how
/// often its strobe flashes (all in seconds of the scene's own time).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Timing {
    pub duration: f64,
    pub slow: f64,
    pub strobe: f64,
}

pub fn timing(scene: Scene) -> Timing {
    match scene {
        Scene::Tape => Timing { duration: 3.2, slow: 1.0, strobe: f64::INFINITY },
        Scene::Cyclist => Timing { duration: 6.0, slow: 1.0, strobe: 1.0 },
        Scene::Overtake => Timing { duration: OVERTAKE_GAP / (OVERTAKE_CAR - OVERTAKE_BUS), slow: 1.0, strobe: 1.0 },
        Scene::GreenLight => Timing { duration: GREEN_TIME, slow: 1.0, strobe: 1.0 },
        Scene::AppleDrop => Timing { duration: (2.0 * BALCONY / EARTH_G).sqrt(), slow: 1.3, strobe: 0.25 },
        Scene::TableEdge => Timing { duration: (2.0 * TABLE / EARTH_G).sqrt(), slow: 4.0, strobe: 0.05 },
        Scene::KickSplit => Timing { duration: kick().1, slow: 1.0, strobe: 0.25 },
        Scene::TwinKicks => Timing { duration: twin(1).1, slow: 1.0, strobe: 0.25 },
    }
}

/// How tall a scene's picture is, in interface points: the tall scenes get more room.
pub fn picture_height(scene: Scene) -> f32 {
    match scene {
        Scene::AppleDrop => 260.0,
        _ => 170.0,
    }
}

/// The part of the scene the picture shows, in metres: left, right, bottom, top.
pub fn view(scene: Scene) -> [f64; 4] {
    match scene {
        Scene::Tape => [-0.45, 2.6, -0.15, 1.15],
        Scene::Cyclist => [-2.0, 33.0, -1.5, 6.0],
        Scene::Overtake => [-8.0, 116.0, -3.0, 22.0],
        Scene::GreenLight => [-6.0, 42.0, -2.0, 15.0],
        Scene::AppleDrop => [-14.0, 16.0, -1.5, 24.5],
        Scene::TableEdge => [-1.3, 1.6, -0.12, 1.25],
        Scene::KickSplit => [-2.0, 36.0, -1.5, 11.0],
        Scene::TwinKicks => [-2.0, 26.0, -1.0, 11.5],
    }
}

const CYCLIST: f64 = 5.0;
/// The car, the bus, and the gap from the car's front to the bus's back when the clock starts.
const OVERTAKE_CAR: f64 = 20.0;
const OVERTAKE_BUS: f64 = 14.0;
const OVERTAKE_GAP: f64 = 30.0;
const CAR_LENGTH: f64 = 4.5;
const BUS_LENGTH: f64 = 12.0;
/// The car pulling away from the lights: its steady acceleration, in m/s², and how long it is shown.
const GREEN_ACCEL: f64 = 3.0;
const GREEN_TIME: f64 = 5.0;
const BALCONY: f64 = 20.0;
const TABLE: f64 = 1.0;
const ROLL: f64 = 1.5;

/// The kick split into its parts, and how long it is in the air.
fn kick() -> (Projectile, f64) {
    let p = Projectile::from_angle(0.0, 0.0, 18.0, 40f64.to_radians(), EARTH_G);
    (p, 2.0 * p.vy / EARTH_G)
}

/// The flat (0) and the steep (1) of two kicks at the same speed, and how long each is in the air.
fn twin(which: usize) -> (Projectile, f64) {
    let angle = [30f64, 60.0][which].to_radians();
    let p = Projectile::from_angle(0.0, 0.0, 16.0, angle, EARTH_G);
    (p, 2.0 * p.vy / EARTH_G)
}

/// Where everything that moves in a scene is, `t` seconds of scene time after it starts.
pub fn tracks(scene: Scene, t: f64) -> Vec<Track> {
    let timing = timing(scene);
    let t = t.clamp(0.0, timing.duration);
    let track = |at: &dyn Fn(f64) -> (f64, f64)| {
        let flashes = if timing.strobe.is_finite() { (t / timing.strobe + 1e-9).floor() as usize } else { 0 };
        Track { now: at(t), ghosts: (0..=flashes).map(|k| at(k as f64 * timing.strobe)).filter(|_| timing.strobe.is_finite()).collect() }
    };
    match scene {
        // The tape's end, unrolling along the sofa.
        Scene::Tape => vec![track(&|t| ((t / 1.2).min(1.0) * 2.0, 0.55))],
        Scene::Cyclist => vec![track(&|t| (CYCLIST * t, 0.0))],
        // The car's front and the bus's back.
        Scene::Overtake => vec![track(&|t| (OVERTAKE_CAR * t, 0.0)), track(&|t| (OVERTAKE_GAP + OVERTAKE_BUS * t, 0.0))],
        // The car's front, speeding up from rest at the stop line.
        Scene::GreenLight => vec![track(&|t| (0.5 * GREEN_ACCEL * t * t, 0.0))],
        // The apple, then the hammer and the feather in the tube beside it.
        Scene::AppleDrop => {
            let fall = |x: f64| move |t: f64| (x, BALCONY - 0.5 * EARTH_G * t * t);
            vec![track(&fall(0.5)), track(&fall(8.0)), track(&fall(10.5))]
        }
        // The rolled marble, then the dropped one.
        Scene::TableEdge => {
            let fall = |t: f64| TABLE - 0.5 * EARTH_G * t * t;
            vec![track(&|t| (ROLL * t, fall(t))), track(&|t| (-0.02, fall(t)))]
        }
        Scene::KickSplit => {
            let (p, _) = kick();
            vec![track(&|t| p.pos(t))]
        }
        Scene::TwinKicks => (0..2)
            .map(|which| {
                let (p, flight) = twin(which);
                track(&|t| p.pos(t.min(flight)))
            })
            .collect(),
    }
}

/// The page's text, its picture, its quiz, and the materials table under all the pages.
pub fn body(ui: &mut egui::Ui, game: &mut Game, ui_state: &mut Ui, room: f32) {
    let now = ui.input(|i| i.time);
    egui::ScrollArea::vertical().id_salt("handbook").max_height(room).show(ui, |ui| {
        ui.label(RichText::new("Pages").strong());
        ui.label(RichText::new("A page is written when you see what it describes happen in the world.").small().color(DIM));
        let focus = ui_state.handbook_focus.take();
        // A page is listed once a puzzle that uses it is open; it can be read once found.
        let mut shown: Vec<String> = Vec::new();
        let ids: Vec<String> = (0..game.chapter.puzzles.len()).filter(|&i| game.unlocked(i)).flat_map(|i| game.def(i).concepts.clone()).collect();
        for id in ids {
            if shown.contains(&id) {
                continue;
            }
            shown.push(id.clone());
            let Some(concept) = game.chapter.concept(&id).cloned() else { continue };
            if !game.page_found(&id) {
                ui.label(RichText::new(format!("{} — not found yet", concept.title)).color(DIM));
                ui.label(RichText::new(format!("    {}", concept.hint)).small().italics().color(DIM));
                continue;
            }
            let focused = focus.as_deref() == Some(id.as_str());
            let mut header = egui::CollapsingHeader::new(page_title(ui, game, ui_state, &concept)).id_salt(("page", &id));
            if focused {
                header = header.open(Some(true));
            }
            let shown_page = header.show(ui, |ui| page(ui, game, ui_state, &concept, now));
            if focused {
                shown_page.header_response.scroll_to_me(Some(egui::Align::TOP));
            }
        }
        ui.separator();
        materials(ui);
    });
}

/// A page's title in the list: marked new until read, and ticked once its quiz is answered.
fn page_title(ui: &egui::Ui, game: &Game, ui_state: &Ui, concept: &Concept) -> egui::text::LayoutJob {
    let font = egui::TextStyle::Body.resolve(ui.style());
    let plain = ui.visuals().text_color();
    let mut job = egui::text::LayoutJob::default();
    let new = ui_state.unread_pages.contains(&concept.id);
    job.append(&concept.title, 0.0, egui::TextFormat::simple(font.clone(), if new { GOOD } else { plain }));
    if new {
        job.append("· new", 8.0, egui::TextFormat::simple(font.clone(), GOOD));
    }
    if game.save.quiz_done(&game.chapter, &concept.id) {
        job.append("✔", 8.0, egui::TextFormat::simple(font, GOOD));
    }
    job
}

fn page(ui: &mut egui::Ui, game: &mut Game, ui_state: &mut Ui, concept: &Concept, now: f64) {
    page_text(ui, &concept.card);
    if let Some(illustration) = &concept.illustration {
        ui.add_space(4.0);
        let anim = ui_state.anims.entry(concept.id.clone()).or_insert(Anim { start: now, paused_at: None });
        picture(ui, illustration, anim, now);
    }
    if let Some(at) = game.chapter.puzzle(concept.found.puzzle()) {
        ui.label(RichText::new(format!("Found at: {}", at.title)).small().color(DIM));
    }
    if !concept.quiz.is_empty() {
        quiz(ui, game, ui_state, concept);
    }
    ui.add_space(6.0);
}

/// A page's words: paragraphs in the reading font, and each formula (a paragraph indented by four
/// spaces) in a box of its own, in a fixed-width font.
pub fn page_text(ui: &mut egui::Ui, card: &str) {
    for paragraph in card.split("\n\n") {
        if paragraph.starts_with("    ") {
            let formula: Vec<&str> = paragraph.lines().map(|l| l.strip_prefix("    ").unwrap_or(l)).collect();
            egui::Frame::new().fill(Color32::from_rgb(24, 30, 42)).corner_radius(4.0).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                ui.label(RichText::new(formula.join("\n")).monospace().size(14.0).color(Color32::from_rgb(255, 217, 120)));
            });
        } else {
            ui.label(paragraph);
        }
        ui.add_space(2.0);
    }
}

/// Where a picture's clock stands, in seconds of scene time: it plays, holds the last frame, and
/// plays again.
fn scene_time(scene: Scene, anim: &Anim, now: f64) -> f64 {
    let timing = timing(scene);
    let wall = anim.paused_at.unwrap_or(now) - anim.start;
    let lap = timing.duration * timing.slow + HOLD;
    (wall.rem_euclid(lap) / timing.slow).min(timing.duration)
}

fn picture(ui: &mut egui::Ui, illustration: &Illustration, anim: &mut Anim, now: f64) {
    let scene = illustration.scene;
    let t = scene_time(scene, anim, now);
    let width = ui.available_width().min(PICTURE_WIDTH);
    let (response, painter) = ui.allocate_painter(egui::vec2(width, picture_height(scene)), egui::Sense::hover());
    let rect = response.rect;
    let [x0, x1, y0, y1] = view(scene);
    let scale = (rect.width() as f64 / (x1 - x0)).min(rect.height() as f64 / (y1 - y0));
    let centre = (rect.width() as f64 - (x1 - x0) * scale) / 2.0;
    let pen = Pen {
        painter: &painter,
        scale: scale as f32,
        at: Box::new(move |x: f64, y: f64| egui::pos2(rect.left() + (centre + (x - x0) * scale) as f32, rect.bottom() - ((y - y0) * scale) as f32)),
    };
    sky(&painter, rect);
    let tracks = tracks(scene, t);
    match scene {
        Scene::Tape => tape(&pen, &tracks, t),
        Scene::Cyclist => cyclist(&pen, &tracks, t),
        Scene::Overtake => overtake(&pen, &tracks, t),
        Scene::GreenLight => green_light(&pen, &tracks, t),
        Scene::AppleDrop => apple_drop(&pen, &tracks),
        Scene::TableEdge => table_edge(&pen, &tracks),
        Scene::KickSplit => kick_split(&pen, &tracks, t),
        Scene::TwinKicks => twin_kicks(&pen, &tracks),
    }
    // The scene's clock, and how much it is slowed.
    let timing = timing(scene);
    if timing.strobe.is_finite() {
        let mut clock = format!("t = {} s", sig_fixed(t, if timing.strobe < 0.1 { 2 } else { 1 }));
        if timing.slow > 1.0 {
            clock.push_str(&format!("   ·   shown {}× slower", sig(timing.slow, 2)));
        }
        // On a pale card, so it reads over whatever the scene draws beneath it.
        let galley = painter.layout_no_wrap(clock, FontId::proportional(12.0), Color32::from_rgb(20, 30, 50));
        let at = rect.left_top() + egui::vec2(6.0, 5.0);
        painter.rect_filled(egui::Rect::from_min_size(at, galley.size() + egui::vec2(8.0, 4.0)), 3.0, Color32::from_rgba_unmultiplied(255, 255, 255, 190));
        painter.galley(at + egui::vec2(4.0, 2.0), galley, Color32::BLACK);
    }
    painter.rect_stroke(rect, 4.0, Stroke::new(1.0, Color32::from_rgb(90, 100, 120)), egui::StrokeKind::Inside);

    ui.horizontal(|ui| {
        let paused = anim.paused_at.is_some();
        if ui.small_button(if paused { "▶" } else { "⏸" }).on_hover_text(if paused { "Play" } else { "Pause" }).clicked() {
            match anim.paused_at.take() {
                // Carry on from where it stopped.
                Some(at) => anim.start += now - at,
                None => anim.paused_at = Some(now),
            }
        }
        if ui.small_button("⟲").on_hover_text("Play from the start").clicked() {
            anim.start = now;
            anim.paused_at = anim.paused_at.map(|_| now);
        }
    });
    // Under the buttons, so it wraps to the page's width instead of stretching it.
    ui.label(RichText::new(&illustration.caption).small().color(DIM));
    if anim.paused_at.is_none() {
        ui.ctx().request_repaint();
    }
}

/// A number with a fixed count of decimals.
fn sig_fixed(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}")
}

/// Draws in a scene's metres.
struct Pen<'a> {
    painter: &'a egui::Painter,
    /// Points per metre.
    scale: f32,
    at: Box<dyn Fn(f64, f64) -> Pos2 + 'a>,
}

impl Pen<'_> {
    fn at(&self, (x, y): (f64, f64)) -> Pos2 {
        (self.at)(x, y)
    }
    fn rect(&self, a: (f64, f64), b: (f64, f64), radius: f32, colour: Color32) {
        self.painter.rect_filled(egui::Rect::from_two_pos(self.at(a), self.at(b)), radius, colour);
    }
    fn line(&self, a: (f64, f64), b: (f64, f64), stroke: Stroke) {
        self.painter.line_segment([self.at(a), self.at(b)], stroke);
    }
    fn dashed(&self, a: (f64, f64), b: (f64, f64), colour: Color32) {
        self.painter.extend(egui::Shape::dashed_line(&[self.at(a), self.at(b)], Stroke::new(1.0, colour), 4.0, 3.0));
    }
    fn text(&self, at: (f64, f64), anchor: Align2, text: impl ToString, size: f32, colour: Color32) {
        self.painter.text(self.at(at), anchor, text.to_string(), FontId::proportional(size), colour);
    }
}

const SKY_TOP: Color32 = Color32::from_rgb(120, 170, 220);
const SKY_LOW: Color32 = Color32::from_rgb(196, 222, 240);
const GRASS: Color32 = Color32::from_rgb(92, 150, 80);
const SOIL: Color32 = Color32::from_rgb(120, 92, 64);
const ASPHALT: Color32 = Color32::from_rgb(70, 72, 78);
const INK: Color32 = Color32::from_rgb(30, 36, 48);
const GHOST: u8 = 90;

fn faded(colour: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), alpha)
}

/// A blue sky shading to haze at the horizon, and a sun.
fn sky(painter: &egui::Painter, rect: egui::Rect) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), SKY_TOP);
    mesh.colored_vertex(rect.right_top(), SKY_TOP);
    mesh.colored_vertex(rect.left_bottom(), SKY_LOW);
    mesh.colored_vertex(rect.right_bottom(), SKY_LOW);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(1, 3, 2);
    painter.add(mesh);
    let sun = rect.right_top() + egui::vec2(-28.0, 24.0);
    painter.circle_filled(sun, 13.0, Color32::from_rgb(255, 236, 160));
    painter.circle_filled(sun, 9.0, Color32::from_rgb(255, 222, 110));
}

/// Grass from `x0` to `x1` with its top at `y`, over a strip of soil.
fn ground(pen: &Pen, x0: f64, x1: f64, y: f64, depth: f64) {
    pen.rect((x0, y), (x1, y - depth), 0.0, SOIL);
    pen.rect((x0, y), (x1, y - depth * 0.35), 0.0, GRASS);
}

fn ball(pen: &Pen, at: (f64, f64), radius: f32, fill: Color32, ghost: bool) {
    let p = pen.at(at) - egui::vec2(0.0, radius);
    if ghost {
        pen.painter.circle(p, radius, faded(fill, GHOST), Stroke::new(1.0, faded(INK, GHOST)));
    } else {
        pen.painter.circle(p, radius, fill, Stroke::new(1.2, INK));
    }
}

fn tape(pen: &Pen, tracks: &[Track], t: f64) {
    let [x0, x1, ..] = view(Scene::Tape);
    // Floor and wall.
    pen.rect((x0, 1.15), (x1, 0.0), 0.0, Color32::from_rgb(232, 220, 200));
    pen.rect((x0, 0.0), (x1, -0.15), 0.0, Color32::from_rgb(150, 110, 78));
    // The sofa: two metres of it.
    let sofa = Color32::from_rgb(70, 110, 150);
    let dark = Color32::from_rgb(54, 88, 124);
    pen.rect((0.0, 0.45), (2.0, 0.1), 4.0, sofa);
    pen.rect((0.05, 0.85), (1.95, 0.42), 6.0, dark);
    pen.rect((-0.12, 0.62), (0.12, 0.1), 5.0, dark);
    pen.rect((1.88, 0.62), (2.12, 0.1), 5.0, dark);
    for x in [0.1, 1.9] {
        pen.rect((x - 0.03, 0.1), (x + 0.03, 0.0), 1.0, INK);
    }
    // The tape, unrolled from its case to the sofa's far end.
    let end = tracks[0].now.0;
    let y = tracks[0].now.1;
    pen.rect((0.0, y + 0.035), (end, y - 0.035), 1.0, Color32::from_rgb(250, 210, 60));
    let ticks = (end * 10.0).floor() as i32;
    for i in 0..=ticks {
        let x = i as f64 / 10.0;
        let long = i % 10 == 0;
        pen.line((x, y + 0.035), (x, y + if long { -0.03 } else { 0.005 }), Stroke::new(1.0, INK));
        if long && i > 0 {
            pen.text((x, y - 0.06), Align2::CENTER_TOP, format!("{i_m} m", i_m = i / 10), 11.0, INK);
        }
    }
    pen.rect((-0.18, y + 0.09), (0.0, y - 0.09), 4.0, Color32::from_rgb(220, 60, 50));
    // What was measured, said two ways.
    if t > 1.4 {
        pen.text((0.4, 1.02), Align2::CENTER_CENTER, "2", 22.0, BAD);
        pen.text((0.4, 0.92), Align2::CENTER_TOP, "✖ two what?", 12.0, BAD);
    }
    if t > 2.0 {
        pen.text((1.6, 1.02), Align2::CENTER_CENTER, "2 m", 22.0, Color32::from_rgb(40, 140, 70));
        pen.text((1.6, 0.92), Align2::CENTER_TOP, "✔ a length", 12.0, Color32::from_rgb(40, 140, 70));
    }
}

fn bike(pen: &Pen, (x, y): (f64, f64), alpha: u8) {
    let ink = faded(INK, alpha);
    let stroke = Stroke::new(1.6, ink);
    let r = 0.36 * pen.scale;
    let back = pen.at((x - 0.55, y)) - egui::vec2(0.0, r);
    let front = pen.at((x + 0.55, y)) - egui::vec2(0.0, r);
    pen.painter.circle_stroke(back, r, stroke);
    pen.painter.circle_stroke(front, r, stroke);
    let seat = pen.at((x - 0.25, y + 1.0));
    let bars = pen.at((x + 0.45, y + 1.05));
    let crank = pen.at((x, y + 0.36));
    pen.painter.line(vec![back, seat, crank, back], stroke);
    pen.painter.line(vec![seat, bars, front], stroke);
    // The rider.
    let shirt = faded(Color32::from_rgb(220, 70, 60), alpha);
    let hip = seat + egui::vec2(0.0, -0.1 * pen.scale);
    let shoulder = pen.at((x + 0.15, y + 1.65));
    pen.painter.line_segment([hip, shoulder], Stroke::new(0.32 * pen.scale, shirt));
    pen.painter.line_segment([shoulder, bars], Stroke::new(1.8, shirt));
    pen.painter.line_segment([hip, crank], Stroke::new(2.2, faded(Color32::from_rgb(40, 50, 90), alpha)));
    pen.painter.circle_filled(pen.at((x + 0.25, y + 1.95)), 0.2 * pen.scale, faded(Color32::from_rgb(235, 190, 150), alpha));
}

fn cyclist(pen: &Pen, tracks: &[Track], _t: f64) {
    let [x0, x1, ..] = view(Scene::Cyclist);
    ground(pen, x0, x1, 0.0, 1.5);
    pen.rect((x0, 0.0), (x1, -0.5), 0.0, ASPHALT);
    // Distance posts every five metres.
    for i in 0..=6 {
        let x = i as f64 * 5.0;
        pen.rect((x - 0.12, 2.2), (x + 0.12, 0.0), 1.0, Color32::from_rgb(240, 240, 235));
        pen.rect((x - 0.12, 2.2), (x + 0.12, 1.9), 1.0, BAD);
        pen.text((x, -0.6), Align2::CENTER_TOP, format!("{} m", i * 5), 10.0, Color32::WHITE);
    }
    let track = &tracks[0];
    for ghost in &track.ghosts {
        bike(pen, *ghost, 70);
        // A tick under each flash: the same distance every second.
        pen.line((ghost.0, 4.2), (ghost.0, 3.6), Stroke::new(2.0, faded(INK, 200)));
    }
    if track.ghosts.len() > 1 {
        let last = track.ghosts.last().map_or(0.0, |g| g.0);
        pen.line((0.0, 3.9), (last, 3.9), Stroke::new(1.0, faded(INK, 160)));
        pen.text((last, 4.4), Align2::CENTER_BOTTOM, "each second", 10.0, INK);
    }
    bike(pen, track.now, 255);
}

fn vehicle(pen: &Pen, front: f64, length: f64, height: f64, body: Color32, windows: usize) {
    let back = front - length;
    let wheel = 0.45;
    pen.rect((back, height), (front, wheel * 0.9), 3.0, body);
    if windows == 0 {
        // A car's cabin.
        pen.rect((back + length * 0.25, height + 1.0), (front - length * 0.25, height - 0.1), 3.0, body);
        pen.rect((back + length * 0.32, height + 0.85), (front - length * 0.3, height + 0.05), 2.0, Color32::from_rgb(190, 220, 240));
    } else {
        let step = length / (windows as f64 + 1.0);
        for i in 0..windows {
            let x = back + step * (i as f64 + 0.5);
            pen.rect((x, height - 0.4), (x + step * 0.8, height - 1.4), 1.5, Color32::from_rgb(190, 220, 240));
        }
    }
    for x in [back + length * 0.18, front - length * 0.18] {
        pen.painter.circle(pen.at((x, wheel)), wheel as f32 * pen.scale, INK, Stroke::new(1.0, Color32::GRAY));
    }
}

fn overtake(pen: &Pen, tracks: &[Track], t: f64) {
    let [x0, x1, ..] = view(Scene::Overtake);
    ground(pen, x0, x1, 0.0, 3.0);
    pen.rect((x0, 0.0), (x1, -1.4), 0.0, ASPHALT);
    let mut x = x0;
    while x < x1 {
        pen.rect((x, -0.6), (x + 3.0, -0.8), 0.0, Color32::from_rgb(230, 230, 220));
        x += 7.0;
    }
    let car = tracks[0].now.0;
    let bus = tracks[1].now.0;
    vehicle(pen, bus + BUS_LENGTH, BUS_LENGTH, 3.2, Color32::from_rgb(240, 190, 40), 5);
    vehicle(pen, car, CAR_LENGTH, 1.2, Color32::from_rgb(210, 50, 50), 0);
    // The gap, measured from the car's front to the bus's back.
    let gap = bus - car;
    if gap > 0.5 {
        let y = 5.0;
        pen.line((car, y), (bus, y), Stroke::new(1.5, INK));
        pen.line((car, y - 0.6), (car, y + 0.6), Stroke::new(1.5, INK));
        pen.line((bus, y - 0.6), (bus, y + 0.6), Stroke::new(1.5, INK));
        pen.text(((car + bus) / 2.0, y + 0.6), Align2::CENTER_BOTTOM, format!("gap {} m", sig_fixed(gap, 0)), 11.0, INK);
    }
    // The gap at each second, side by side: it shrinks by the same step every time.
    let bars: Vec<f64> = tracks[0].ghosts.iter().zip(&tracks[1].ghosts).map(|(c, b)| b.0 - c.0).collect();
    let (left, base, width) = (x0 + 3.0, 9.0, 3.2);
    pen.text((left, base + 12.0), Align2::LEFT_BOTTOM, "gap, each second", 10.0, INK);
    for (i, gap) in bars.iter().enumerate() {
        let x = left + i as f64 * (width + 1.2);
        pen.rect((x, base + gap / OVERTAKE_GAP * 10.0), (x + width, base), 1.0, faded(Color32::from_rgb(60, 80, 140), 220));
    }
    pen.line((left - 0.5, base), (left + 6.0 * (width + 1.2), base), Stroke::new(1.0, INK));
    if t >= timing(Scene::Overtake).duration {
        pen.text(((car + bus) / 2.0, 7.5), Align2::CENTER_BOTTOM, "caught up", 12.0, Color32::from_rgb(40, 140, 70));
    }
}

fn green_light(pen: &Pen, tracks: &[Track], t: f64) {
    let [x0, x1, ..] = view(Scene::GreenLight);
    ground(pen, x0, x1, 0.0, 2.0);
    pen.rect((x0, 0.0), (x1, -1.2), 0.0, ASPHALT);
    let mut x = 3.0;
    while x < x1 {
        pen.rect((x, -0.5), (x + 3.0, -0.7), 0.0, Color32::from_rgb(230, 230, 220));
        x += 7.0;
    }
    // The stop line, and the lights just turned green.
    pen.rect((0.2, 0.0), (0.6, -1.2), 0.0, Color32::from_rgb(240, 240, 235));
    pen.rect((-3.1, 4.4), (-2.9, 0.0), 0.0, INK);
    pen.rect((-3.6, 6.4), (-2.4, 4.2), 2.0, Color32::from_rgb(40, 44, 52));
    let lamp = 0.4 * pen.scale;
    pen.painter.circle_filled(pen.at((-3.0, 5.85)), lamp, Color32::from_rgb(90, 40, 40));
    pen.painter.circle_filled(pen.at((-3.0, 4.75)), lamp, Color32::from_rgb(70, 230, 110));
    let red = Color32::from_rgb(210, 50, 50);
    let track = &tracks[0];
    // Where the car was at each flash: a faded body and a tick above it. The gaps grow.
    for ghost in &track.ghosts {
        pen.rect((ghost.0 - CAR_LENGTH, 1.2), (ghost.0, 0.4), 3.0, faded(red, 60));
        pen.line((ghost.0, 3.1), (ghost.0, 2.5), Stroke::new(2.0, faded(INK, 200)));
    }
    if track.ghosts.len() > 1 {
        let last = track.ghosts.last().map_or(0.0, |g| g.0);
        pen.line((0.0, 2.8), (last, 2.8), Stroke::new(1.0, faded(INK, 160)));
    }
    let front = track.now.0;
    vehicle(pen, front, CAR_LENGTH, 1.2, red, 0);
    pen.text((front - CAR_LENGTH / 2.0, 3.6), Align2::CENTER_BOTTOM, format!("v = {} m/s", sig_fixed(GREEN_ACCEL * t, 1)), 11.0, INK);
    // Its speed at each second, side by side: it rises by the same step every time.
    let (left, base, width, tall) = (17.0, 8.5, 2.2, 4.5);
    let top = GREEN_ACCEL * GREEN_TIME;
    pen.text((left, base + tall + 0.6), Align2::LEFT_BOTTOM, "speed each second, m/s", 10.0, INK);
    for (k, _) in track.ghosts.iter().enumerate() {
        let v = GREEN_ACCEL * k as f64 * timing(Scene::GreenLight).strobe;
        let x = left + k as f64 * (width + 0.8);
        pen.rect((x, base + v / top * tall), (x + width, base), 1.0, faded(Color32::from_rgb(60, 80, 140), 220));
        pen.text((x + width / 2.0, base - 0.2), Align2::CENTER_TOP, sig_fixed(v, 0), 9.0, INK);
    }
    pen.line((left - 0.5, base), (left + 6.0 * (width + 0.8), base), Stroke::new(1.0, INK));
}

fn apple(pen: &Pen, at: (f64, f64), ghost: bool) {
    ball(pen, at, 5.0, Color32::from_rgb(214, 40, 40), ghost);
    if !ghost {
        let top = pen.at(at) - egui::vec2(0.0, 10.0);
        pen.painter.line_segment([top, top + egui::vec2(3.0, -4.0)], Stroke::new(1.5, Color32::from_rgb(60, 120, 40)));
    }
}

fn apple_drop(pen: &Pen, tracks: &[Track]) {
    let [x0, x1, ..] = view(Scene::AppleDrop);
    ground(pen, x0, x1, 0.0, 1.5);
    // A brick house with a balcony at the drop height.
    let brick = Color32::from_rgb(170, 90, 70);
    pen.rect((-12.0, 23.5), (-0.6, 0.0), 0.0, brick);
    for floor in 0..7 {
        let y = 1.2 + floor as f64 * 3.2;
        for x in [-10.5, -6.5, -3.5] {
            pen.rect((x, y + 1.8), (x + 1.8, y), 1.0, Color32::from_rgb(190, 215, 235));
        }
    }
    pen.rect((-0.6, BALCONY), (1.6, BALCONY - 0.4), 0.0, Color32::from_rgb(120, 120, 128));
    pen.line((1.6, BALCONY), (1.6, BALCONY + 1.2), Stroke::new(1.5, INK));
    pen.line((-0.6, BALCONY + 1.2), (1.6, BALCONY + 1.2), Stroke::new(1.5, INK));
    // The tube with the air pumped out.
    let glass = Color32::from_rgba_unmultiplied(220, 240, 255, 90);
    pen.rect((6.6, BALCONY + 1.5), (12.0, 0.0), 6.0, glass);
    pen.painter.rect_stroke(egui::Rect::from_two_pos(pen.at((6.6, BALCONY + 1.5)), pen.at((12.0, 0.0))), 6.0, Stroke::new(1.2, INK), egui::StrokeKind::Inside);
    pen.text((9.3, BALCONY + 1.7), Align2::CENTER_BOTTOM, "no air", 10.0, INK);
    // Ticks beside each flash of the apple: the gaps grow all the way down.
    let apple_track = &tracks[0];
    for ghost in &apple_track.ghosts {
        apple(pen, *ghost, true);
        pen.line((2.4, ghost.1), (3.6, ghost.1), Stroke::new(1.5, INK));
    }
    apple(pen, apple_track.now, false);
    for ghost in &tracks[1].ghosts {
        hammer(pen, *ghost, GHOST);
    }
    hammer(pen, tracks[1].now, 255);
    for ghost in &tracks[2].ghosts {
        feather(pen, *ghost, GHOST);
    }
    feather(pen, tracks[2].now, 255);
}

fn hammer(pen: &Pen, at: (f64, f64), alpha: u8) {
    let p = pen.at(at);
    pen.painter.line_segment([p + egui::vec2(0.0, -2.0), p + egui::vec2(0.0, -16.0)], Stroke::new(2.5, faded(Color32::from_rgb(150, 100, 60), alpha)));
    pen.painter.rect_filled(egui::Rect::from_center_size(p + egui::vec2(0.0, -16.0), egui::vec2(12.0, 5.0)), 1.0, faded(Color32::from_rgb(90, 95, 105), alpha));
}

fn feather(pen: &Pen, at: (f64, f64), alpha: u8) {
    let p = pen.at(at);
    let quill = faded(Color32::from_rgb(250, 250, 245), alpha);
    pen.painter.line_segment([p + egui::vec2(-1.0, -1.0), p + egui::vec2(2.0, -17.0)], Stroke::new(1.0, faded(INK, alpha)));
    for i in 0..6 {
        let base = p + egui::vec2(0.3 + i as f32 * 0.4, -4.0 - i as f32 * 2.2);
        pen.painter.line_segment([base, base + egui::vec2(-4.5, 1.5)], Stroke::new(1.5, quill));
        pen.painter.line_segment([base, base + egui::vec2(4.5, 0.5)], Stroke::new(1.5, quill));
    }
}

fn table_edge(pen: &Pen, tracks: &[Track]) {
    let [x0, x1, ..] = view(Scene::TableEdge);
    pen.rect((x0, 1.25), (x1, 0.0), 0.0, Color32::from_rgb(232, 220, 200));
    pen.rect((x0, 0.0), (x1, -0.12), 0.0, Color32::from_rgb(150, 110, 78));
    let wood = Color32::from_rgb(140, 96, 60);
    pen.rect((-1.2, TABLE), (0.0, TABLE - 0.06), 1.0, wood);
    for x in [-1.12, -0.12] {
        pen.rect((x, TABLE - 0.06), (x + 0.06, 0.0), 1.0, wood);
    }
    let (rolled, dropped) = (&tracks[0], &tracks[1]);
    // A level line through each pair of flashes: at every moment, the same height.
    for (r, d) in rolled.ghosts.iter().zip(&dropped.ghosts) {
        pen.dashed((d.0 - 0.25, r.1 + 0.02), (r.0 + 0.2, r.1 + 0.02), faded(INK, 150));
    }
    let blue = Color32::from_rgb(60, 120, 220);
    let green = Color32::from_rgb(60, 170, 90);
    for g in &rolled.ghosts {
        ball(pen, *g, 5.0, blue, true);
    }
    for g in &dropped.ghosts {
        ball(pen, *g, 5.0, green, true);
    }
    ball(pen, rolled.now, 5.0, blue, false);
    ball(pen, dropped.now, 5.0, green, false);
    pen.text((0.95, 1.12), Align2::CENTER_CENTER, "rolled off", 11.0, blue);
    pen.text((-0.12, 0.55), Align2::RIGHT_CENTER, "dropped", 11.0, green);
}

fn kick_split(pen: &Pen, tracks: &[Track], t: f64) {
    let [x0, x1, ..] = view(Scene::KickSplit);
    ground(pen, x0, x1, 0.0, 1.5);
    let track = &tracks[0];
    for g in &track.ghosts {
        ball(pen, *g, 4.0, Color32::WHITE, true);
        // Below each flash, a tick on the ground: the sideways steps are all the same.
        pen.line((g.0, -0.1), (g.0, -0.7), Stroke::new(1.5, faded(Color32::WHITE, 200)));
    }
    ball(pen, track.now, 5.0, Color32::WHITE, false);
    let (p, flight) = kick();
    if t < flight {
        let (vx, vy) = p.vel(t);
        // Arrows from the ball: the sideways part, the upward part, and the whole velocity.
        let s = pen.scale * 0.25;
        let from = pen.at(track.now) - egui::vec2(0.0, 5.0);
        let arrow = |v: Vec2, colour: Color32, label: &str| {
            pen.painter.arrow(from, v, Stroke::new(2.2, colour));
            pen.painter.text(from + v + v.normalized() * 8.0, Align2::CENTER_CENTER, label, FontId::proportional(11.0), colour);
        };
        arrow(egui::vec2(vx as f32 * s, 0.0), Color32::from_rgb(30, 70, 200), "vx");
        if vy.abs() > 0.5 {
            arrow(egui::vec2(0.0, -vy as f32 * s), Color32::from_rgb(20, 130, 50), "vy");
        }
        arrow(egui::vec2(vx as f32 * s, -vy as f32 * s), INK, "v");
    }
    // The kicker's foot at the start.
    pen.rect((-1.2, 0.5), (-0.2, 0.0), 2.0, Color32::from_rgb(40, 40, 50));
}

fn twin_kicks(pen: &Pen, tracks: &[Track]) {
    let [x0, x1, ..] = view(Scene::TwinKicks);
    ground(pen, x0, x1, 0.0, 1.0);
    let colours = [Color32::from_rgb(245, 140, 30), Color32::from_rgb(40, 120, 220)];
    // Where both land: a flag.
    let (p, flight) = twin(0);
    let land = p.pos(flight).0;
    pen.line((land, 0.0), (land, 2.4), Stroke::new(1.5, INK));
    pen.rect((land, 2.4), (land + 1.2, 1.7), 0.0, BAD);
    for (track, colour) in tracks.iter().zip(colours) {
        for g in &track.ghosts {
            ball(pen, *g, 4.0, colour, true);
        }
        ball(pen, track.now, 5.0, colour, false);
    }
    pen.text((2.2, 0.9), Align2::LEFT_CENTER, "30°", 12.0, colours[0]);
    pen.text((0.4, 2.6), Align2::LEFT_CENTER, "60°", 12.0, colours[1]);
}

/// The page's questions. Optional: they change nothing in the world, and a tick on the page shows
/// they have all been answered right.
fn quiz(ui: &mut egui::Ui, game: &mut Game, ui_state: &mut Ui, concept: &Concept) {
    let done = game.save.quiz_done(&game.chapter, &concept.id);
    let title = if done { RichText::new("Check yourself  ✔ all answered").color(GOOD) } else { RichText::new("Check yourself (optional)") };
    let mut header = egui::CollapsingHeader::new(title).id_salt(("quiz", &concept.id));
    let focused = ui_state.quiz_focus.as_deref() == Some(concept.id.as_str());
    if focused {
        header = header.open(Some(true));
        ui_state.quiz_focus = None;
    }
    let shown = header.show(ui, |ui| {
        for (q, question) in concept.quiz.iter().enumerate() {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{}. {}", q + 1, question.question)).strong());
            let key = (concept.id.clone(), q);
            let picked = ui_state.quiz_picks.get(&key).copied();
            for (i, choice) in question.choices.iter().enumerate() {
                if ui.radio(picked == Some(i), choice.as_str()).clicked() {
                    pick(game, ui_state, &concept.id, q, i);
                }
            }
            match picked {
                Some(i) if i == question.answer => {
                    ui.label(RichText::new(format!("✔ Right. {}", question.explain)).color(GOOD));
                }
                Some(_) => {
                    ui.label(RichText::new("✖ Not quite. Have another look at the page, and try again.").color(BAD));
                }
                None => {}
            }
        }
    });
    if focused {
        shown.header_response.scroll_to_me(Some(egui::Align::TOP));
    }
}

/// Choose an answer to a quiz question, as a click on it does. A right one is kept in the save.
pub fn pick(game: &mut Game, ui_state: &mut Ui, concept_id: &str, question: usize, choice: usize) {
    ui_state.quiz_picks.insert((concept_id.to_string(), question), choice);
    let right = game.chapter.concept(concept_id).and_then(|c| c.quiz.get(question)).is_some_and(|q| q.answer == choice);
    if right {
        game.save.answer_quiz(&game.chapter, concept_id, question);
        game.persist();
    }
}

fn materials(ui: &mut egui::Ui) {
    ui.label(RichText::new("Materials (one block is one cubic metre)").strong());
    egui::Grid::new("materials").num_columns(5).striped(true).show(ui, |ui| {
        for heading in ["", "density\nkg/m³", "specific heat\nJ/(kg K)", "conductivity\nW/(m K)", "resistivity\nohm m"] {
            ui.label(RichText::new(heading).small().color(DIM));
        }
        ui.end_row();
        for material in Material::ALL {
            let p = material.properties();
            ui.label(p.name);
            ui.label(sig(p.density, 3));
            ui.label(sig(p.specific_heat, 3));
            ui.label(sig(p.thermal_conductivity, 2));
            ui.label(if p.resistivity.is_finite() { format!("{:.1e}", p.resistivity) } else { "insulator".into() });
            ui.end_row();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [Scene; 8] = [Scene::Tape, Scene::Cyclist, Scene::Overtake, Scene::GreenLight, Scene::AppleDrop, Scene::TableEdge, Scene::KickSplit, Scene::TwinKicks];

    fn samples(scene: Scene) -> impl Iterator<Item = f64> {
        let duration = timing(scene).duration;
        (0..=100).map(move |i| duration * i as f64 / 100.0)
    }

    #[test]
    fn every_scene_stays_in_its_picture() {
        for scene in ALL {
            let [x0, x1, y0, y1] = view(scene);
            for t in samples(scene) {
                for track in tracks(scene, t) {
                    for (x, y) in std::iter::once(track.now).chain(track.ghosts) {
                        assert!((x0..=x1).contains(&x) && (y0..=y1).contains(&y), "{scene:?} at {t} s: ({x}, {y}) is outside the picture");
                    }
                }
            }
        }
    }

    #[test]
    fn the_rolled_and_the_dropped_marble_are_level_at_every_flash() {
        let end = tracks(Scene::TableEdge, timing(Scene::TableEdge).duration);
        let (rolled, dropped) = (&end[0], &end[1]);
        assert!(rolled.ghosts.len() >= 8, "enough flashes to see it");
        for (r, d) in rolled.ghosts.iter().zip(&dropped.ghosts) {
            assert!((r.1 - d.1).abs() < 1e-9);
        }
        assert!(rolled.now.0 > 0.5, "the rolled one lands well out from the table");
        assert!(rolled.now.1.abs() < 1e-9 && dropped.now.1.abs() < 1e-9, "both reach the floor at the end");
    }

    #[test]
    fn the_flat_and_the_steep_kick_land_on_the_same_spot() {
        let end = tracks(Scene::TwinKicks, timing(Scene::TwinKicks).duration);
        assert!((end[0].now.0 - end[1].now.0).abs() < 0.01, "{:?}", end.iter().map(|t| t.now).collect::<Vec<_>>());
        assert!(end.iter().all(|t| t.now.1.abs() < 1e-9));
        // The steep one stays up longer.
        assert!(twin(1).1 > twin(0).1);
    }

    #[test]
    fn the_car_gains_the_same_speed_every_second() {
        let end = tracks(Scene::GreenLight, timing(Scene::GreenLight).duration);
        let strobe = timing(Scene::GreenLight).strobe;
        let gaps: Vec<f64> = end[0].ghosts.windows(2).map(|w| w[1].0 - w[0].0).collect();
        assert!(gaps.len() >= 4, "{gaps:?}");
        // Each gap is longer than the last by the same step: a steady acceleration.
        let steps: Vec<f64> = gaps.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(steps.iter().all(|s| (s - GREEN_ACCEL * strobe * strobe).abs() < 1e-9), "{steps:?}");
        assert!(end[0].ghosts[0] == (0.0, 0.0), "it starts from rest at the stop line");
    }

    #[test]
    fn the_falling_apple_covers_more_in_each_flash() {
        let end = tracks(Scene::AppleDrop, timing(Scene::AppleDrop).duration);
        let gaps: Vec<f64> = end[0].ghosts.windows(2).map(|w| w[0].1 - w[1].1).collect();
        assert!(gaps.len() >= 6);
        assert!(gaps.windows(2).all(|w| w[1] > w[0]), "{gaps:?}");
        // The hammer and the feather keep pace with each other.
        assert_eq!(end[1].ghosts.iter().map(|g| g.1).collect::<Vec<_>>(), end[2].ghosts.iter().map(|g| g.1).collect::<Vec<_>>());
    }

    #[test]
    fn steady_motion_covers_equal_steps() {
        let end = tracks(Scene::Cyclist, timing(Scene::Cyclist).duration);
        let gaps: Vec<f64> = end[0].ghosts.windows(2).map(|w| w[1].0 - w[0].0).collect();
        assert!(gaps.len() >= 5 && gaps.iter().all(|g| (g - CYCLIST).abs() < 1e-9), "{gaps:?}");
        // The car closes on the bus by the same step each second, and meets it at the end.
        let end = tracks(Scene::Overtake, timing(Scene::Overtake).duration);
        let gaps: Vec<f64> = end[0].ghosts.iter().zip(&end[1].ghosts).map(|(c, b)| b.0 - c.0).collect();
        let steps: Vec<f64> = gaps.windows(2).map(|w| w[0] - w[1]).collect();
        assert!(steps.iter().all(|s| (s - (OVERTAKE_CAR - OVERTAKE_BUS)).abs() < 1e-9), "{steps:?}");
        assert!((end[1].now.0 - end[0].now.0).abs() < 1e-9);
    }

    #[test]
    fn the_kick_keeps_its_sideways_speed() {
        let end = tracks(Scene::KickSplit, timing(Scene::KickSplit).duration);
        let steps: Vec<f64> = end[0].ghosts.windows(2).map(|w| w[1].0 - w[0].0).collect();
        assert!(steps.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-9));
        assert!(end[0].now.1.abs() < 1e-9, "it lands at the end");
    }

    #[test]
    fn a_picture_plays_holds_and_plays_again() {
        let scene = Scene::Cyclist;
        let timing = timing(scene);
        let anim = Anim { start: 10.0, paused_at: None };
        assert_eq!(scene_time(scene, &anim, 10.0), 0.0);
        assert!((scene_time(scene, &anim, 12.0) - 2.0).abs() < 1e-9);
        assert_eq!(scene_time(scene, &anim, 10.0 + timing.duration + HOLD / 2.0), timing.duration, "it holds the end");
        assert!(scene_time(scene, &anim, 10.0 + timing.duration + HOLD + 1.0) < 1.0 + 1e-9, "then plays again");
        let paused = Anim { start: 10.0, paused_at: Some(11.0) };
        assert_eq!(scene_time(scene, &paused, 50.0), 1.0, "a paused picture stays where it stopped");
    }

    #[test]
    fn formulas_are_set_apart_from_the_words() {
        // The renderer's rule: a paragraph indented by four spaces is a formula.
        for concept in physics_core::content::chapter_1_1().concepts {
            let formulas = concept.card.split("\n\n").filter(|p| p.starts_with("    ")).count();
            let words = concept.card.split("\n\n").filter(|p| !p.starts_with("    ")).count();
            assert!(words >= 1, "{} has words", concept.id);
            if concept.id != "units" {
                assert!(formulas >= 1, "{} sets its relation apart", concept.id);
            }
        }
    }
}
