//! Scripted runs for automated checks. Development only; enabled by `GAME_SCRIPT`.
//!
//! - `tour` plays chapter 1.1, tutorial first, with the expressions a player would type and writes
//!   screenshots. It loads the cart, times the crawler and catches it, clamps the light gate and times
//!   drops at two notches, and stacks the channel's blocks.
//! - `walk` drives the avatar with key presses: up the steps, off the edge of the ledge into the deep
//!   water of the channel and back onto the ledge, over the bridge once it stands, a jump.
//! - `pose` films the avatar up close: standing, walking, running, jumping, at a console, watching a
//!   test, and its reactions to a miss and a pass.
//! - `intro` photographs the intro behind the title menu, presses Start and checks that the game
//!   takes over: the camera ends on the player's view, turns to the main hull and the skiff while
//!   Lumen names them, and the avatar walks.
//! - `beam` plays the beam off the road: carries a block from its pile onto a seat, releases it
//!   short and then level, sets the next two cases' blocks, and photographs it waking and its page.
//! - `prefill` is not a script: you play by hand, with the tour's ruler readings in the notebook and
//!   its formulas in the blueprints (see `prefill`).
//!
//! `tour`, `walk` and `pose` press Start first, as a player would.
//! The scripts go through the same requests as the interface (blueprint text, commit, ruler picks,
//! key state), so they exercise the real paths rather than setting results directly. A script that
//! does not see what it expects panics, which fails the run.

use crate::avatar::{Activity, AvatarState};
use crate::carry::{self, Carry, DirtyCells, Slot, Target};
use crate::companion::{Companion, station_name};
use crate::intro::Intro;
use crate::island::Sites;
use crate::player::{OrbitCamera, Player, Shore};
use crate::ruler::{Ruler, pick_at};
use crate::sim::{Request, Sim};
use crate::state::{Game, Mode, Ui};
use crate::voxel::VoxelWorld;
use crate::waking::{Timeline, Waking};
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use physics_core::notebook::source;
use physics_core::units::Quantity;
use std::collections::VecDeque;
use std::path::PathBuf;

enum Step {
    Wait(f32),
    /// Wait until no experiment or test is running.
    WaitIdle,
    /// Stand beside a station's console.
    GoToStation(usize),
    GoTo(Vec3),
    Look {
        yaw: f32,
        pitch: f32,
        distance: f32,
    },
    /// Point the camera at a fixed spot instead of the avatar.
    Watch(Option<Vec3>),
    Station(Option<usize>),
    /// Aim the ruler at a survey pin, as a click on it would.
    RulerPin(usize),
    /// Press one of the ruler's "note as" buttons: 0 horizontal, 1 vertical.
    RulerNote(usize),
    RulerOn,
    RulerOff,
    /// Wait until Lumen looks at this place; fail if it has not happened in time.
    ExpectGaze {
        place: String,
        within: f32,
    },
    /// Pick a choice in a station's experiment, as a click on it would.
    Guess(usize, usize),
    /// Press the experiment's run button.
    RunDemo(usize),
    /// Wait until a handbook page has been written; fail if it has not happened in time.
    ExpectFound {
        page: &'static str,
        within: f32,
    },
    /// Open the handbook at a page, as its button at the station would.
    OpenPage(&'static str),
    /// Answer a question of a handbook page's quiz, rightly or not, as a click on a choice would.
    Quiz {
        page: &'static str,
        question: usize,
        right: bool,
    },
    /// Fail unless every question of the page's quiz has been answered right.
    ExpectQuizDone(&'static str),
    Draft(usize, Vec<String>),
    Commit(usize),
    /// Stand at `feet` facing along `facing`, with the pointer marking `aim` on the drop tower, and
    /// press F; fail unless F would do what is expected.
    CarryAt {
        feet: Vec3,
        facing: Vec3,
        expect: Expect,
        aim: Option<f32>,
    },
    /// Fail unless the last commit was refused for this reason.
    ExpectRefusal(&'static str),
    /// Fail if a handbook page has been written.
    ExpectNotFound(&'static str),
    /// Press the ledge launcher's rating plate's Note button.
    NotePlate,
    /// Type into the calculator.
    Calculate(String),
    /// Fail unless the puzzle is solved.
    ExpectSolved(usize),
    Windows {
        notebook: bool,
        handbook: bool,
        tutor: bool,
    },
    Shot(&'static str),
    /// Press or release a key, as the keyboard would.
    Key(KeyCode, bool),
    /// Wait until the avatar stands at this height; fail if it has not happened in time.
    UntilStandingAt {
        height: f32,
        within: f32,
        what: &'static str,
    },
    /// Wait until the avatar stands north of this row (smaller z); fail if it has not in time.
    UntilNorthOf {
        z: f32,
        within: f32,
        what: &'static str,
    },
    /// Wait until the avatar has fallen into deep water; fail if it has not in time.
    UntilFallenIn {
        within: f32,
    },
    /// Fail unless the avatar stands at this height now.
    ExpectStanding {
        height: f32,
        what: &'static str,
    },
    /// Wait until the avatar is in the air above this height.
    UntilAirborneAbove {
        height: f32,
        within: f32,
    },
    /// Wait until the avatar's animation shows this activity; fail if it has not happened in time.
    ExpectActivity {
        activity: Activity,
        within: f32,
    },
    /// Press Start on the title menu.
    Start,
    /// Wait until the game has taken over from the menu.
    UntilPlaying {
        within: f32,
    },
    /// Wait until the intro is this many seconds into its loop.
    UntilIntro(f32),
    /// Check that the camera stands where the player's view puts it.
    ExpectPlayerView,
    /// Wait until the avatar has moved a metre from where it stood when the step began.
    UntilMoved {
        from: Option<Vec3>,
        within: f32,
    },
    /// Mark the first `n` puzzles solved, as passes would, without playing them.
    Solve(usize),
    /// Wait until no wake is playing; fail if one still is after this many seconds.
    WaitWakeEnd(f32),
    /// Wait until the way shows this stage; fail if it has not happened in time.
    UntilWoken {
        stage: usize,
        within: f32,
    },
    /// Wait until the wake to this stage has played this many seconds; fail if it has not in time.
    UntilWakeAt {
        stage: usize,
        at: f32,
        within: f32,
    },
    /// Open or close the beam's console.
    BeamConsole(bool),
    /// Press the beam's Release button.
    Release,
    /// Wait until the beam has swung this many seconds (or, with `None`, until it is pinned again).
    UntilBeam {
        at: Option<f32>,
        within: f32,
    },
    /// Put these blocks on the beam's seats (as the save keeps them), as carrying them would.
    SetBeam(Vec<u32>),
    /// Fail unless this many of the beam's cases have held.
    ExpectHeld(u32),
    Quit,
}

/// What a press of F should do.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Expect {
    /// Pick up a loose block.
    Take,
    TakeGate,
    PutIn(Slot),
    TakeOut(Slot),
    /// Clamp the light gate at this notch.
    Clamp(u32),
    /// Put the block on this seat of the beam.
    OnBeam(physics_core::beam::Arm, u32),
}

impl Expect {
    fn matches(self, target: Option<Target>) -> bool {
        match (self, target) {
            (Expect::Take, Some(Target::Take(_))) | (Expect::TakeGate, Some(Target::TakeGate)) => true,
            (Expect::PutIn(slot), Some(Target::PutIn(s, _))) | (Expect::TakeOut(slot), Some(Target::TakeOut(s, _))) => s == slot,
            (Expect::Clamp(notch), Some(Target::PutIn(Slot::Tower, n))) => n == notch,
            (Expect::OnBeam(arm, seat), Some(Target::PutOnBeam(a, s))) => (a, s) == (arm, seat),
            _ => false,
        }
    }
}

/// Stand at `feet` facing along `facing` and press F.
fn carry_at(feet: Vec3, facing: Vec3, expect: Expect) -> Step {
    Step::CarryAt { feet, facing, expect, aim: None }
}

/// Where to stand to pick up the cart's block, and to load it onto the cart.
fn by_the_rail(sites: &Sites) -> (Vec3, Vec3) {
    let cell = sites.cargo_cell;
    (Vec3::new(cell.x as f32 - 0.6, cell.y as f32, cell.z as f32 + 0.5), sites.rail_start - Vec3::X * 1.2)
}

#[derive(Resource)]
struct Script {
    steps: VecDeque<Step>,
    wait: f32,
    shots: PathBuf,
}

pub struct DevScriptPlugin;

impl Plugin for DevScriptPlugin {
    fn build(&self, app: &mut App) {
        let Ok(name) = std::env::var("GAME_SCRIPT") else { return };
        if name == "prefill" {
            app.add_plugins(crate::prefill::PrefillPlugin);
            return;
        }
        let game = app.world().resource::<Game>();
        let sites = app.world().resource::<Sites>();
        let steps = match name.as_str() {
            "tour" => tour(game, sites),
            "walk" => walk(game, sites),
            "intro" => intro(),
            "pose" => pose(game, sites),
            "way" => way(sites),
            "beam" => beam(game, sites),
            other => panic!("unknown GAME_SCRIPT '{other}' (known: tour, walk, intro, pose, way, beam, prefill)"),
        };
        let shots = PathBuf::from(std::env::var("GAME_SHOTS").unwrap_or_else(|_| "shots".into()));
        std::fs::create_dir_all(&shots).expect("the screenshot directory can be created");
        app.insert_resource(Script { steps: steps.into(), wait: 2.0, shots }).add_systems(Update, run);
    }
}

/// Index of the haul track's first pin (the cart's front coupler; then the crawler, the marker and the
/// two posts): its pins are added after every other.
const HAUL_PINS: usize = 13;
/// The camera's yaw at the haul track: from the beach, south-west of the station, looking along it.
const HAUL_YAW: f32 = -0.65;
/// The launchers fire north: a view of one is turned this far from the view it would have firing east.
const TURN: f32 = std::f32::consts::FRAC_PI_2;

/// Where to stand at the jack frame's foot, on the ledge launcher's rail, facing it.
fn at_the_frame(sites: &Sites) -> (Vec3, Vec3) {
    let out = sites.channel_launch.direction;
    (sites.channel_well.as_vec3() + Vec3::new(0.5, 0.0, 0.5) - out * 1.4, out)
}

fn tour(game: &Game, sites: &Sites) -> Vec<Step> {
    use Step::*;
    let (min, max) = game.layout.ridge_angle_window();
    let angle = ((min + max) / 2.0).to_degrees().round();
    let closed = Windows { notebook: false, handbook: false, tutor: false };
    let (cargo_at, cart_at) = by_the_rail(sites);
    // The light gate: off its stand, then clamped at a short drop and a long one.
    let at_stand = sites.gate_stand - Vec3::X * 1.0;
    let at_tower = sites.tower_plate + Vec3::Z * 1.8;
    let (short, long) = (3, game.layout.tower_h - 2);
    let clamp =
        |notch: u32| Step::CarryAt { feet: at_tower, facing: Vec3::NEG_Z, expect: Expect::Clamp(notch), aim: Some(sites.tower_release.y - notch as f32) };
    // When things happen in each wake, to photograph them.
    let wakes: Vec<Timeline> = (1..=crate::state::PUZZLES).map(|stage| Timeline::new(stage, &sites.way)).collect();
    let mut steps = vec![
        Start,
        UntilPlaying { within: 10.0 },
        Shot("01_start"),
        // Station 0 is behind a close camera: Lumen's beam ends in an arrow at the screen's edge.
        expect_gaze(station_name(game, 0)),
        Look { yaw: -0.8, pitch: 0.15, distance: 3.0 },
        Wait(1.2),
        Shot("01b_lumen_points_off_screen"),
        // The whole route, from the wreck on the south coast to the skiff on the north shore.
        // The route's northern half, from the channel to the skiff on the north shore, looked at from
        // high over it so no hill comes between the camera and it (the camera keeps within 80 m).
        Watch(Some(sites.tower_plate.lerp(sites.slipway, 0.35).with_y(crate::island::GROUND as f32 + 45.0))),
        Look { yaw: 0.25, pitch: 1.0, distance: 80.0 },
        Wait(1.5),
        Shot("02_island"),
        Watch(None),
        // Puzzle 0, the tutorial: measure the rail, a bare number is refused, the reading passes.
        GoToStation(0),
        Look { yaw: 0.35, pitch: 0.5, distance: 22.0 },
        Wait(1.0),
        expect_gaze(station_name(game, 0)),
        Station(Some(0)),
        // The cart will not run empty: refused, for free, and no page is written for it.
        Draft(0, vec![game.layout.rail_target.to_string()]),
        Commit(0),
        Wait(0.3),
        ExpectRefusal("Load the cart first"),
        ExpectNotFound("units"),
        expect_gaze("The cart's block beside the rail"),
        Shot("03a_cart_empty"),
        Station(None),
        carry_at(cargo_at, Vec3::X, Expect::Take),
        expect_gaze("Rail cart"),
        Look { yaw: 1.2, pitch: 0.3, distance: 6.0 },
        Wait(0.8),
        Shot("03b_carrying_the_cargo"),
        carry_at(cart_at, Vec3::X, Expect::PutIn(Slot::Cart)),
        Wait(0.5),
        Shot("03c_cart_loaded"),
        GoToStation(0),
        Look { yaw: 0.35, pitch: 0.5, distance: 22.0 },
        Station(Some(0)),
        RulerOn,
        expect_gaze("Rail cart: starting point"),
        RulerPin(10),
        expect_gaze("Rail marker: centre"),
        RulerPin(11),
        Wait(0.3),
        Shot("03_ruler_along_rail"),
        RulerNote(0),
        RulerOff,
        Draft(0, vec![game.layout.rail_target.to_string()]),
        Commit(0),
        Wait(0.3),
        Shot("04_bare_number_refused"),
        // The first value committed at the rail writes the handbook's first page, refused or not.
        ExpectFound { page: "units", within: 2.0 },
        Draft(0, vec!["d1".into()]),
        Commit(0),
        Wait(1.5),
        Shot("05_rail_run"),
        WaitIdle,
        Wait(3.0),
        Shot("06_rail_result"),
        // Stepping away from the console with E, the way wakes (and the E that closed the console does
        // not skip it): the marker's studs light, the light runs up the road, the stone gate opens
        // for the cart, and the haul track's console lights.
        Key(KeyCode::KeyE, true),
        Wait(0.1),
        Key(KeyCode::KeyE, false),
        UntilWakeAt { stage: 1, at: 0.8, within: 30.0 },
        Shot("06_wake_marker_lights"),
        UntilWakeAt { stage: 1, at: 1.0 + wakes[0].wake_starts() * 0.5, within: 30.0 },
        Shot("06_wake_light_runs"),
        UntilWakeAt { stage: 1, at: wakes[0].wake_starts() + 1.8, within: 30.0 },
        Shot("06_wake_stone_gate_opens"),
        UntilWakeAt { stage: 1, at: wakes[0].console_lights() + 0.4, within: 30.0 },
        Shot("06_wake_console_lights"),
        WaitWakeEnd(30.0),
        Windows { notebook: false, handbook: true, tutor: false },
        Wait(0.4),
        Shot("06b_first_page_and_a_locked_one"),
        OpenPage("units"),
        Wait(2.9),
        Shot("06b2_page_units"),
        closed_step(),
        // Puzzle 1: time the crawler, measure the track, catch it too soon, then on the marker.
        GoToStation(1),
        // Away from the solved station, Lumen points ahead to the next one.
        expect_gaze(station_name(game, 1)),
        Look { yaw: HAUL_YAW, pitch: 0.5, distance: 34.0 },
        Wait(1.0),
        Station(Some(1)),
        expect_gaze("Crawler: rear coupler"),
        Shot("06c_haul_track"),
        // The crawler carries the launchers' sight on its back.
        Watch(Some(sites.haul_at(game.layout.head_start as f64 + 0.7))),
        Look { yaw: HAUL_YAW, pitch: 0.35, distance: 6.0 },
        Wait(1.0),
        Shot("06c2_sight_on_the_crawler"),
        Watch(None),
        Look { yaw: HAUL_YAW, pitch: 0.5, distance: 34.0 },
        Guess(1, 0),
        RunDemo(1),
        Wait(4.0),
        Shot("06d_crawler_and_cart_pace"),
        WaitIdle,
        ExpectFound { page: "relative_velocity", within: 2.0 },
        Wait(0.5),
        Shot("06e_pace_seen"),
        OpenPage("velocity"),
        Wait(3.2),
        Shot("06e2_page_velocity"),
        OpenPage("relative_velocity"),
        Wait(3.2),
        Shot("06e3_page_relative_velocity"),
        closed_step(),
        Look { yaw: HAUL_YAW, pitch: 0.5, distance: 34.0 },
        Wait(3.0),
        // Head start, the marker, and the posts: d2, d3, d4. The stopwatch noted t1.
        RulerOn,
        RulerPin(HAUL_PINS),
        RulerPin(HAUL_PINS + 1),
        RulerNote(0),
        RulerOn,
        RulerPin(HAUL_PINS),
        RulerPin(HAUL_PINS + 2),
        RulerNote(0),
        RulerOn,
        RulerPin(HAUL_PINS + 3),
        RulerPin(HAUL_PINS + 4),
        Wait(0.3),
        Shot("06f_ruler_between_posts"),
        RulerNote(0),
        RulerOff,
        Windows { notebook: true, handbook: false, tutor: false },
        Calculate("u = d4/t1".into()),
        Wait(0.5),
        Shot("06g_crawler_speed"),
        closed_step(),
        // Twice the crawler's speed closes the gap in no time: it is caught well before the marker.
        Draft(1, vec!["2*d4/t1".into()]),
        Commit(1),
        WaitIdle,
        Wait(2.0),
        Shot("06h_caught_too_soon"),
        Draft(1, vec!["(d4/t1)*d3/(d3 - d2)".into()]),
        Commit(1),
        Wait(5.0),
        Shot("06i_chase"),
        WaitIdle,
        Wait(2.5),
        ExpectSolved(1),
        Shot("06j_chase_result"),
        Station(None),
        // The crane wakes and lifts the sight off the crawler, round, and down at its foot.
        UntilWakeAt { stage: 2, at: 0.5, within: 30.0 },
        Shot("06j1_wake_crane_lifts_the_sight"),
        UntilWakeAt { stage: 2, at: 1.6, within: 30.0 },
        Shot("06j1b_wake_crane_swings"),
        UntilWakeAt { stage: 2, at: wakes[1].wake_starts() + 1.2, within: 30.0 },
        Shot("06j2_wake_the_tower_foot_lights"),
        WaitWakeEnd(30.0),
        // Caught: the sight waits at the foot of the transfer crane.
        Watch(Some(sites.crane_foot)),
        // From above, so the crane's mast does not stand between the camera and the unit.
        Look { yaw: 2.6, pitch: 1.1, distance: 9.0 },
        Wait(1.0),
        Shot("06k_sight_at_the_crane"),
        Watch(None),
        // Puzzle 2: measure the tower, time the drop, work out g.
        GoToStation(2),
        expect_gaze(station_name(game, 2)),
        Look { yaw: 0.9, pitch: 0.35, distance: 26.0 },
        Wait(1.0),
        Station(Some(2)),
        // The experiment needs the light gate: Lumen points at it on its stand.
        expect_gaze("Light gate on its stand"),
        Wait(1.2),
        Shot("07a_try_it_first"),
        RulerPin(0),
        RulerPin(1),
        Wait(0.3),
        Shot("07_ruler_on_tower"),
        RulerNote(1),
        RulerOff,
        // The gate first, at a short drop.
        Station(None),
        carry_at(at_stand, Vec3::X, Expect::TakeGate),
        expect_gaze("Notches on the drop tower"),
        clamp(short),
        Look { yaw: 0.6, pitch: 0.25, distance: 12.0 },
        Wait(0.8),
        Shot("08c_gate_clamped"),
        GoToStation(2),
        Look { yaw: 0.9, pitch: 0.35, distance: 26.0 },
        Station(Some(2)),
        // With the gate on, Lumen points at the experiment until it has been watched.
        expect_gaze("Drop tower: release point"),
        Guess(2, 1),
        RunDemo(2),
        Wait(1.6),
        Shot("08_strobe_drop"),
        WaitIdle,
        ExpectFound { page: "acceleration", within: 2.0 },
        ExpectFound { page: "free_fall", within: 2.0 },
        Wait(0.6),
        Shot("08b_strobe_seen"),
        // The pages the experiment wrote: a car pulling away from traffic lights, then an apple falling
        // on Earth and its quiz.
        OpenPage("acceleration"),
        Wait(5.5),
        Shot("08b2_page_acceleration"),
        OpenPage("free_fall"),
        Wait(1.1),
        Shot("08f_handbook_free_fall"),
        Step::Quiz { page: "free_fall", question: 0, right: false },
        Wait(0.4),
        Shot("08g_quiz_wrong"),
        Step::Quiz { page: "free_fall", question: 0, right: true },
        Step::Quiz { page: "free_fall", question: 1, right: true },
        Step::Quiz { page: "free_fall", question: 2, right: true },
        ExpectQuizDone("free_fall"),
        Wait(0.6),
        Shot("08h_quiz_done"),
        closed_step(),
        // Then a long drop: the gate moves down the tower, and the experiment runs again.
        Station(None),
        carry_at(at_tower, Vec3::NEG_Z, Expect::TakeOut(Slot::Tower)),
        clamp(long),
        GoToStation(2),
        Station(Some(2)),
        RunDemo(2),
        Wait(1.4),
        Shot("08d_gate_tripped"),
        WaitIdle,
        Wait(0.4),
        Shot("08e_gate_reading"),
        Windows { notebook: true, handbook: false, tutor: false },
        Draft(2, vec!["2*h3/t3^2".into()]),
        Wait(0.3),
        Shot("09_gravity_blueprint"),
        closed_step(),
        Commit(2),
        Wait(1.0),
        Shot("10_mast_test"),
        WaitIdle,
        Wait(3.0),
        Shot("11_gravity_result"),
        Station(None),
        // The tower lights to the top, then the ramp rises.
        UntilWakeAt { stage: 3, at: 0.9, within: 30.0 },
        Shot("11_wake_tower_to_the_top"),
        UntilWakeAt { stage: 3, at: wakes[2].wake_starts() + 1.2, within: 30.0 },
        Shot("11_wake_ramp_rises"),
        WaitWakeEnd(30.0),
        // Puzzle 3: a refused entry, a miss, then the calculated shot.
        GoToStation(3),
        Look { yaw: -1.05 + TURN, pitch: 0.45, distance: 30.0 },
        Wait(1.0),
        Station(Some(3)),
        // A wrong guess, on purpose: the dropped block first.
        Guess(3, 0),
        RunDemo(3),
        Wait(1.5),
        Shot("11a_drop_and_fire"),
        WaitIdle,
        ExpectFound { page: "independence", within: 2.0 },
        Wait(0.5),
        Shot("11b_drop_and_fire_landed"),
        OpenPage("independence"),
        Wait(1.0),
        Shot("11b2_page_independence"),
        closed_step(),
        Wait(3.0),
        RulerPin(4),
        RulerPin(5),
        Wait(0.3),
        Shot("12_ruler_across_channel"),
        RulerNote(0),
        RulerNote(1),
        RulerOff,
        // The rating plate, then the lowest height above the ledge the band allows, in the calculator.
        NotePlate,
        Windows { notebook: true, handbook: false, tutor: false },
        Calculate("hmin = g*d5^2/(2*vmax^2) - h4".into()),
        Wait(0.5),
        Shot("13_calculator_rise"),
        closed_step(),
        // Straight from the bare ledge first: the speed it needs is past the top of the dial.
        Draft(3, vec![String::new(), "d5/sqrt(2*h4/g)".into()]),
        Commit(3),
        Wait(0.3),
        Shot("13b_dial_refuses"),
        Station(None),
    ];
    // Then the stack worked out, and the speed for it.
    let stack = *game.layout.channel_stacks().start();
    let pile_west = sites.block_pile.iter().map(|c| c.x).min().expect("there is a pile") as f32;
    let pile_row = sites.block_pile.iter().map(|c| c.z).min().expect("there is a pile") as f32;
    let ledge = sites.channel_well.y as f32;
    let at_pile = Vec3::new(pile_west - 0.6, ledge, pile_row + 0.5);
    let (at_frame, to_frame) = at_the_frame(sites);
    for k in 0..stack {
        steps.push(carry_at(at_pile, Vec3::X, Expect::Take));
        if k == 0 {
            steps.extend([Look { yaw: 2.3 + TURN, pitch: 0.35, distance: 7.0 }, Wait(0.8), Shot("14a_carrying")]);
        }
        steps.push(carry_at(at_frame, to_frame, Expect::PutIn(Slot::Frame)));
    }
    steps.extend([
        Look { yaw: 2.5 + TURN, pitch: 0.12, distance: 17.0 },
        Wait(1.0),
        Shot("14b_launcher_raised"),
        GoToStation(3),
        Look { yaw: -1.4 + TURN, pitch: 0.45, distance: 30.0 },
        Station(Some(3)),
        Draft(3, vec![String::new(), format!("d5/sqrt(2*(h4 + {stack} m)/g)")]),
        Wait(0.5),
        Shot("14c_blueprint"),
        Commit(3),
        Wait(1.2),
        Shot("15_channel_flight"),
        WaitIdle,
        Wait(2.5),
        ExpectSolved(3),
        Shot("16_channel_result"),
        Station(None),
        UntilWakeAt { stage: 4, at: 0.6, within: 30.0 },
        Shot("16_wake_banks_light"),
        UntilWakeAt { stage: 4, at: 2.6, within: 30.0 },
        Shot("16a_wake_the_bridge_rises"),
        UntilWakeAt { stage: 4, at: wakes[3].wake_starts() + 1.4, within: 30.0 },
        Shot("16b_wake_the_tilting_launcher"),
        WaitWakeEnd(30.0),
        // The casket is down on the platform, and the tiller waits on the tilting launcher beside it.
        Watch(Some(sites.ridge_launch.origin - sites.ridge_launch.direction * 1.5)),
        Look { yaw: -0.6 + TURN, pitch: 0.55, distance: 18.0 },
        Wait(3.0),
        Shot("16c_tiller_on_the_tilting_launcher"),
        Watch(None),
        // Puzzle 4: 45 degrees strikes the ridge; an angle inside the window clears it.
        GoToStation(4),
        Look { yaw: -1.15 + TURN, pitch: 0.4, distance: 34.0 },
        Wait(1.0),
        Station(Some(4)),
        Guess(4, 2),
        RunDemo(4),
        Wait(1.4),
        Shot("16a_two_angles"),
        WaitIdle,
        ExpectFound { page: "range", within: 2.0 },
        Wait(0.5),
        Shot("16b_two_angles_landed"),
        OpenPage("components"),
        Wait(1.2),
        Shot("16b2_page_components"),
        OpenPage("range"),
        Wait(1.6),
        Shot("16b3_page_range"),
        closed_step(),
        Wait(3.0),
        RulerPin(6),
        RulerPin(9),
        RulerNote(0),
        RulerOff,
        Draft(4, vec!["45 deg".into(), "sqrt(g*d6)".into()]),
        Commit(4),
        WaitIdle,
        Wait(2.0),
        Shot("17_ridge_struck"),
        Draft(4, vec![format!("{angle} deg"), format!("sqrt(g*d6/sin(2*{angle} deg))")]),
        // The launcher shows the blueprint before anything is committed: the arm tilts, the gauge fills.
        Watch(Some(sites.ridge_launch.origin + sites.ridge_launch.direction * 1.5 + Vec3::Y * 1.2)),
        Look { yaw: -0.75 + TURN, pitch: 0.2, distance: 9.5 },
        Wait(1.5),
        Shot("17b_launcher_shows_the_blueprint"),
        Watch(None),
        Look { yaw: -1.15 + TURN, pitch: 0.4, distance: 34.0 },
        Wait(0.5),
        Commit(4),
        Wait(1.8),
        Shot("18_ridge_flight"),
        WaitIdle,
        Wait(3.0),
        Shot("19_ridge_result"),
        Station(None),
        UntilWakeAt { stage: 5, at: 2.4, within: 30.0 },
        Shot("19a_wake_the_seal_sinks"),
        UntilWakeAt { stage: 5, at: wakes[4].wake_starts() + 2.2, within: 30.0 },
        Shot("19_wake_north_gate"),
        WaitWakeEnd(30.0),
        expect_gaze("The skiff on the slipway"),
        GoTo(sites.slipway + Vec3::new(9.0, 0.0, 11.0)),
        Look { yaw: -0.8 + TURN, pitch: 0.3, distance: 16.0 },
        Wait(3.5),
        Shot("20_helm_in_the_skiff"),
        Windows { notebook: true, handbook: true, tutor: true },
        OpenPage("range"),
        Wait(0.5),
        Shot("21_notebook_handbook_lumen"),
        closed,
        Quit,
    ]);
    steps
}

fn walk(game: &Game, sites: &Sites) -> Vec<Step> {
    use Step::*;
    let ground = crate::island::GROUND as f32;
    let ledge = sites.channel_launch.origin;
    let back = -sites.channel_launch.direction;
    let (north, east) = (0.0, -std::f32::consts::FRAC_PI_2);
    // The bridge runs north across the channel: start on the near bank, a few steps short of it.
    let deck = sites.way.bridge.centre();
    let half = game.layout.channel_width as f32 / 2.0;
    let bridge_foot = Vec3::new(deck.x, ground, deck.z + half + 4.0);
    let far_bank = deck.z - half - 2.0;
    vec![
        Start,
        UntilPlaying { within: 10.0 },
        // The steps onto the outcrop rise once gravity is measured: wake the way that far.
        Solve(3),
        UntilWoken { stage: 3, within: 240.0 },
        WaitWakeEnd(60.0),
        // Start on the grass south of the steps and walk north: up the steps onto the outcrop.
        GoTo((ledge + back * (20.0 + game.layout.ledge_h as f32)).with_y(ground)),
        Look { yaw: north, pitch: 0.5, distance: 16.0 },
        Wait(0.5),
        Key(KeyCode::KeyW, true),
        expect_activity(Activity::Walk),
        UntilStandingAt { height: ledge.y, within: 12.0, what: "climb the steps to the top of the outcrop" },
        Shot("walk_01_on_the_ledge"),
        // Keep walking: the avatar walks off the edge of the ledge into the deep water of the channel,
        // and is back on the ledge where it stepped off.
        UntilFallenIn { within: 8.0 },
        Key(KeyCode::KeyW, false),
        Wait(0.5),
        ExpectStanding { height: ledge.y, what: "come back to the edge of the ledge after falling into the channel" },
        Shot("walk_02_back_at_the_edge"),
        // Once the casket is across, the bridge stands: walk over it to the north island.
        Solve(4),
        UntilWoken { stage: 4, within: 240.0 },
        WaitWakeEnd(60.0),
        GoTo(bridge_foot),
        Look { yaw: north, pitch: 0.5, distance: 16.0 },
        Wait(0.5),
        Key(KeyCode::KeyW, true),
        UntilNorthOf { z: far_bank, within: 20.0, what: "cross the bridge" },
        Key(KeyCode::KeyW, false),
        Wait(0.3),
        ExpectStanding { height: ground, what: "stand on the north island" },
        Shot("walk_03_over_the_bridge"),
        Look { yaw: east, pitch: 0.5, distance: 16.0 },
        // A jump leaves the ground and comes back to it.
        Key(KeyCode::Space, true),
        UntilAirborneAbove { height: ground + 0.5, within: 2.0 },
        expect_activity(Activity::Air),
        Key(KeyCode::Space, false),
        UntilStandingAt { height: ground, within: 4.0, what: "land after the jump" },
        expect_activity(Activity::Idle),
        Shot("walk_04_a_jump"),
        Quit,
    ]
}

fn intro() -> Vec<Step> {
    use Step::*;
    vec![
        UntilIntro(3.0),
        Shot("intro_01_orbit"),
        UntilIntro(7.0),
        Shot("intro_02_breakup"),
        UntilIntro(18.6),
        Shot("intro_03_mainland"),
        UntilIntro(24.5),
        Shot("intro_04_strait"),
        UntilIntro(29.15),
        Shot("intro_05_impact"),
        UntilIntro(33.0),
        Shot("intro_06_rising"),
        UntilIntro(41.0),
        Shot("intro_07_castaway"),
        Start,
        UntilPlaying { within: 8.0 },
        Wait(0.5),
        ExpectPlayerView,
        Shot("intro_08_playing"),
        // Lumen's opening names the main hull, then the skiff: the camera turns to show each across
        // the bay.
        ExpectGaze { place: "The main hull, across the strait".into(), within: 15.0 },
        Wait(2.5),
        Shot("intro_09_main_hull"),
        ExpectGaze { place: "The skiff on the slipway".into(), within: 25.0 },
        Wait(2.5),
        Shot("intro_10_skiff"),
        Key(KeyCode::KeyW, true),
        UntilMoved { from: None, within: 3.0 },
        Key(KeyCode::KeyW, false),
        Quit,
    ]
}

/// The way's machines, one picture each: asleep in a new game, then with every puzzle solved.
fn way(sites: &Sites) -> Vec<Step> {
    use Step::*;
    let way = &sites.way;
    let mut steps = vec![Start, UntilPlaying { within: 10.0 }, Windows { notebook: false, handbook: false, tutor: false }];
    let views: [(&'static str, Vec3, f32, f32, f32); 10] = [
        ("01_stone_gate", way.stone_gate, 0.9, 0.35, 16.0),
        ("02_crane", way.crane_pivot.lerp(sites.crane_foot, 0.5), 0.6, 0.4, 16.0),
        ("03_tower", sites.tower_plate, 0.7, 0.3, 26.0),
        ("04_ramp", sites.channel_launch.origin - sites.channel_launch.direction * 18.0, 2.6, 0.45, 26.0),
        ("05_channel", sites.channel_launch.origin, 1.9, 0.5, 34.0),
        ("05b_bridge", way.bridge.centre(), -0.8, 0.4, 36.0),
        ("06_pad", sites.ridge_launch.origin.lerp(sites.slipway, 0.4), 1.6, 0.6, 50.0),
        ("06b_seal", way.seal.centre(), 1.2, 0.4, 24.0),
        ("07_north_gate", way.north_gate, 0.25, 0.25, 30.0),
        ("08_skiff", sites.slipway, 2.3, 0.45, 20.0),
    ];
    let photograph = |steps: &mut Vec<Step>, state: &str| {
        for (name, at, yaw, pitch, distance) in views {
            let shot: &'static str = Box::leak(format!("way_{name}_{state}").into_boxed_str());
            steps.extend([Watch(Some(at + Vec3::Y * 2.0)), Look { yaw, pitch, distance }, Wait(1.2), Shot(shot)]);
        }
        steps.push(Watch(None));
    };
    photograph(&mut steps, "asleep");
    // Four wakes as they play: the light running, the machine waking, its console lighting, the
    // camera on its way home. At the casket's pass the bridge rises first, while both banks light,
    // and at the tiller's the seal sinks first, while the landing circle lights.
    for stage in [1, 3, 4, 5] {
        let wake = Timeline::new(stage, way);
        let mut moments = vec![wake.wake_starts() * 0.6, wake.wake_starts() + 0.6, wake.console_lights() + 0.4, wake.length() - 0.5];
        if stage >= 4 {
            moments.splice(0..0, [1.2, 2.4, 3.6]);
        }
        steps.extend([Solve(stage - 1), UntilWoken { stage: stage - 1, within: 240.0 }, WaitWakeEnd(240.0), Solve(stage)]);
        for (i, at) in moments.into_iter().enumerate() {
            let shot: &'static str = Box::leak(format!("way_wake_{stage}_{i}").into_boxed_str());
            steps.extend([UntilWakeAt { stage, at, within: 120.0 }, Shot(shot)]);
        }
        steps.extend([UntilWoken { stage, within: 60.0 }, WaitWakeEnd(60.0)]);
    }
    photograph(&mut steps, "awake");
    steps.push(Quit);
    steps
}

/// Blocks on the far arm only that balance `load`, from the outermost seat in, as the save keeps them.
fn balancing(load: physics_core::beam::Load) -> Vec<u32> {
    use physics_core::beam::{Arm, MOST_ON_A_SEAT, Placement, SEATS};
    let mut left = load.turning();
    let mut placement = Placement::default();
    for seat in (1..=SEATS).rev() {
        let n = (left / seat).min(MOST_ON_A_SEAT);
        placement.set(Arm::Far, seat, n);
        left -= n * seat;
    }
    assert_eq!(left, 0, "{load:?} is balanced on the far arm alone");
    placement.to_saved()
}

fn beam(game: &Game, sites: &Sites) -> Vec<Step> {
    use Step::*;
    use physics_core::beam::{Arm, cases};
    let beam = &sites.beam;
    let ground = beam.pivot.y - crate::island::BEAM_AXLE;
    let cases = cases(game.save.seed);
    // The pile's top block, from the west, and the far arm's first seat from the south.
    let top = *beam.pile.iter().max_by_key(|c| (c.y, -c.x, c.z)).expect("there is a pile");
    let at_pile = Vec3::new(top.x as f32 - 0.6, ground, top.z as f32 + 0.5);
    let at_seat = |seat: u32| Vec3::new(beam.seat(Arm::Far, seat).x, ground, beam.pivot.z + 1.4);
    let overview = |steps: &mut Vec<Step>, name: &'static str| {
        steps.extend([Watch(Some(beam.pivot + Vec3::new(3.0, 0.5, 0.0))), Look { yaw: 0.45, pitch: 0.35, distance: 17.0 }, Wait(1.0), Shot(name), Watch(None)]);
    };
    let mut steps = vec![Start, UntilPlaying { within: 10.0 }, Windows { notebook: false, handbook: false, tutor: false }, GoTo(at_seat(2)), Wait(1.0)];
    overview(&mut steps, "beam_01_asleep");
    // One block onto the first seat of the far arm: too little for any load.
    steps.extend([
        carry_at(at_pile, Vec3::X, Expect::Take),
        Look { yaw: 0.6, pitch: 0.3, distance: 7.0 },
        Wait(0.6),
        Shot("beam_02_carrying"),
        carry_at(at_seat(1), Vec3::NEG_Z, Expect::OnBeam(Arm::Far, 1)),
        GoTo(beam.console.as_vec3() + Vec3::new(0.5, 0.0, 1.6)),
        BeamConsole(true),
        Wait(2.0),
        Shot("beam_03_console"),
        Release,
        UntilBeam { at: Some(2.2), within: 10.0 },
        Shot("beam_04_load_down"),
        UntilBeam { at: None, within: 10.0 },
        ExpectHeld(0),
    ]);
    // Each case balanced, as a player who has found the rule would build it.
    for (k, load) in cases.iter().enumerate() {
        steps.extend([SetBeam(balancing(*load)), Wait(0.5), Release, UntilBeam { at: Some(2.0), within: 10.0 }]);
        steps.push(Shot(["beam_05_level_1", "beam_06_level_2", "beam_07_level_3"][k]));
        steps.extend([UntilBeam { at: None, within: 10.0 }, ExpectHeld(k as u32 + 1)]);
        if k == 1 {
            steps.extend([Wait(0.5), Shot("beam_08_last_load")]);
        }
    }
    steps.extend([Wait(0.6), BeamConsole(false), Wait(0.4)]);
    overview(&mut steps, "beam_09_waking");
    steps.extend([ExpectFound { page: "a_rule_predicts", within: 2.0 }, Wait(3.0)]);
    overview(&mut steps, "beam_10_awake");
    steps.extend([OpenPage("a_rule_predicts"), Wait(2.5), Shot("beam_11_page"), Quit]);
    steps
}

fn pose(game: &Game, sites: &Sites) -> Vec<Step> {
    use Step::*;
    let pile_west = sites.block_pile.iter().map(|c| c.x).min().expect("there is a pile") as f32;
    let pile_row = sites.block_pile.iter().map(|c| c.z).min().expect("there is a pile") as f32;
    let at_pile = Vec3::new(pile_west - 0.6, sites.channel_well.y as f32, pile_row + 0.5);
    let (at_frame, to_frame) = at_the_frame(sites);
    let (cargo_at, cart_at) = by_the_rail(sites);
    let close = Look { yaw: 0.0, pitch: 0.2, distance: 4.5 };
    let at_console = || Look { yaw: 0.9, pitch: 0.2, distance: 4.5 };
    vec![
        Start,
        UntilPlaying { within: 10.0 },
        GoTo(sites.spawn),
        close,
        Wait(1.5),
        expect_activity(Activity::Idle),
        Shot("pose_01_idle"),
        // Sideways to the camera, so the legs are seen in profile.
        Key(KeyCode::KeyD, true),
        expect_activity(Activity::Walk),
        Wait(0.45),
        Shot("pose_02_walk"),
        Key(KeyCode::ShiftLeft, true),
        expect_activity(Activity::Run),
        Wait(0.3),
        Shot("pose_03_run"),
        Key(KeyCode::ShiftLeft, false),
        Key(KeyCode::KeyD, false),
        Wait(0.6),
        Key(KeyCode::Space, true),
        expect_activity(Activity::Air),
        Key(KeyCode::Space, false),
        Wait(0.1),
        Shot("pose_04_jump"),
        expect_activity(Activity::Idle),
        // A block from the ledge's pile, carried.
        carry_at(at_pile, Vec3::X, Expect::Take),
        Look { yaw: 2.3 + TURN, pitch: 0.25, distance: 4.5 },
        Wait(0.8),
        Shot("pose_09_carrying"),
        Key(KeyCode::KeyS, true),
        Wait(0.45),
        Shot("pose_10_carrying_walk"),
        Key(KeyCode::KeyS, false),
        // Hands free again before the console: the block goes into the jack frame.
        carry_at(at_frame, to_frame, Expect::PutIn(Slot::Frame)),
        Wait(0.5),
        // The cart will not run empty.
        carry_at(cargo_at, Vec3::X, Expect::Take),
        carry_at(cart_at, Vec3::X, Expect::PutIn(Slot::Cart)),
        Wait(0.3),
        // At the rail's console: working the panel, watching the cart, a miss and then a pass.
        GoToStation(0),
        at_console(),
        Station(Some(0)),
        // The station turns the camera to the rail; look back at the avatar.
        Wait(0.2),
        Watch(None),
        at_console(),
        expect_activity(Activity::Typing),
        Wait(1.2),
        Shot("pose_05_typing"),
        Draft(0, vec!["3 m".into()]),
        Commit(0),
        expect_activity(Activity::Watching),
        Watch(None),
        at_console(),
        Wait(0.8),
        Shot("pose_06_watching"),
        expect_activity(Activity::Slump),
        Wait(0.8),
        Shot("pose_07_miss"),
        WaitIdle,
        expect_activity(Activity::Typing),
        Draft(0, vec![format!("{} m", game.layout.rail_target)]),
        Commit(0),
        expect_activity(Activity::Cheer),
        Watch(None),
        at_console(),
        Wait(0.2),
        Shot("pose_08_pass"),
        Quit,
    ]
}

fn expect_activity(activity: Activity) -> Step {
    Step::ExpectActivity { activity, within: 10.0 }
}

fn expect_gaze(place: impl Into<String>) -> Step {
    Step::ExpectGaze { place: place.into(), within: 8.0 }
}

fn closed_step() -> Step {
    Step::Windows { notebook: false, handbook: false, tutor: false }
}

/// The rest of what a step touches, in one parameter: a system takes at most sixteen.
type MoreOfTheGame<'w, 's> = (
    Option<ResMut<'w, Intro>>,
    Res<'w, State<Mode>>,
    Single<'w, 's, &'static AvatarState>,
    ResMut<'w, Carry>,
    ResMut<'w, DirtyCells>,
    Res<'w, Waking>,
    Res<'w, Shore>,
    ResMut<'w, crate::beam::Beam>,
);

#[allow(clippy::too_many_arguments)]
fn run(
    time: Res<Time>,
    mut commands: Commands,
    mut script: ResMut<Script>,
    mut game: ResMut<Game>,
    mut ui: ResMut<Ui>,
    mut sim: ResMut<Sim>,
    mut ruler: ResMut<Ruler>,
    mut orbit: ResMut<OrbitCamera>,
    mut world: ResMut<VoxelWorld>,
    mut sites: ResMut<Sites>,
    mut player: Single<(&mut Player, &mut Transform), Without<Camera3d>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    companion: Res<Companion>,
    mut exit: MessageWriter<AppExit>,
    (mut intro, mode, avatar, mut carry, mut dirty, waking, shore, mut beam): MoreOfTheGame,
) {
    if script.wait > 0.0 {
        script.wait -= time.delta_secs();
        return;
    }
    let Some(step) = script.steps.pop_front() else { return };
    match step {
        Step::Wait(seconds) => script.wait = seconds,
        Step::WaitWakeEnd(within) => {
            if waking.playing() {
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "the wake did not end");
                script.steps.push_front(Step::WaitWakeEnd(left));
            }
        }
        Step::Solve(n) => {
            for puzzle in 0..n {
                let id = game.def(puzzle).id.clone();
                game.save.puzzles.entry(id).or_default().solved = true;
            }
            game.persist();
        }
        Step::UntilWakeAt { stage, at, within } => {
            if !waking.play.is_some_and(|p| p.timeline.stage == stage && p.t >= at) {
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "the wake to stage {stage} did not reach {at} s: {:?}", waking.play.map(|p| (p.timeline.stage, p.t)));
                script.steps.push_front(Step::UntilWakeAt { stage, at, within: left });
            }
        }
        Step::UntilWoken { stage, within } => {
            if waking.shown != stage {
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "the way did not wake to stage {stage}: it shows {}", waking.shown);
                script.steps.push_front(Step::UntilWoken { stage, within: left });
            }
        }
        Step::WaitIdle => {
            if sim.busy() || sim.request.is_some() {
                script.steps.push_front(Step::WaitIdle);
            }
        }
        Step::GoToStation(i) => {
            player.1.translation = sites.stations[i].as_vec3() + Vec3::new(-1.5, 0.0, 0.5);
            player.0.velocity = Vec3::ZERO;
        }
        Step::GoTo(position) => {
            player.1.translation = position;
            player.0.velocity = Vec3::ZERO;
        }
        Step::Look { yaw, pitch, distance } => orbit.set_view(yaw, pitch, distance),
        Step::Watch(target) => orbit.watch = target,
        Step::Station(station) => ui.station = station,
        Step::RulerPin(i) => {
            ruler.active = true;
            let (camera, camera_transform) = *camera;
            let pin = &sites.pins[i];
            let cursor = camera.world_to_viewport(camera_transform, pin.position).unwrap_or_else(|e| panic!("pin '{}' is off screen: {e:?}", pin.label));
            let picked = pick_at(&world, &sites, camera, camera_transform, cursor).expect("the ruler finds something under the cursor");
            assert_eq!(picked.label, pin.label, "the ruler snapped to the wrong thing");
            ruler.pick(picked);
        }
        Step::RulerNote(which) => {
            let m = ruler.measurement().expect("two points are picked");
            let (prefix, value) = [("d", m.horizontal), ("h", m.vertical)][which];
            game.save.notebook.add_numbered(prefix, "measured by the script", Quantity::metres(value as f64), source::RULER);
        }
        Step::RulerOn => ruler.start(),
        Step::ExpectGaze { place, within } => {
            if companion.gaze.as_deref() != Some(place.as_str()) {
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "Lumen did not look at '{place}': it looks at {:?}", companion.gaze);
                script.steps.push_front(Step::ExpectGaze { place, within: left });
            }
        }
        Step::RulerOff => ruler.put_away(),
        Step::Guess(puzzle, choice) => ui.guess[puzzle] = Some(choice),
        Step::RunDemo(puzzle) => {
            assert!(game.save.demo_watched(&game.def(puzzle).id) || ui.guess[puzzle].is_some(), "the experiment wants a guess before its first run");
            sim.request = Some(Request::Demo(puzzle));
        }
        Step::ExpectFound { page, within } => {
            if !game.save.found.contains(page) {
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "the handbook page '{page}' was not written");
                script.steps.push_front(Step::ExpectFound { page, within: left });
            }
        }
        Step::OpenPage(page) => ui.open_page(page),
        Step::BeamConsole(open) => ui.beam_open = open,
        Step::Release => {
            assert!(beam.swinging.is_none(), "the beam is already swinging");
            crate::beam::release_now(&mut beam, &game, &mut carry);
        }
        Step::UntilBeam { at, within } => {
            let there = match (at, beam.swinging) {
                (Some(at), Some(s)) => s.t >= at,
                (Some(_), None) => false,
                (None, swinging) => swinging.is_none(),
            };
            if !there {
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "the beam did not get to {at:?}");
                script.steps.push_front(Step::UntilBeam { at, within: left });
            }
        }
        Step::SetBeam(saved) => {
            carry.beam = physics_core::beam::Placement::from_saved(&saved);
            game.save.sides.entry(crate::beam::BEAM_ID.to_string()).or_default().placed = saved;
            game.persist();
        }
        Step::ExpectHeld(n) => assert_eq!(crate::beam::held(&game), n, "the beam's cases held"),
        Step::Quiz { page, question, right } => {
            let concept = game.chapter.concept(page).unwrap_or_else(|| panic!("no page '{page}'"));
            let asked = &concept.quiz[question];
            let choice = if right { asked.answer } else { (asked.answer + 1) % asked.choices.len() };
            ui.open_page(page);
            ui.quiz_focus = Some(page.to_string());
            crate::handbook::pick(&mut game, &mut ui, page, question, choice);
        }
        Step::ExpectQuizDone(page) => assert!(game.save.quiz_done(&game.chapter, page), "the quiz on '{page}' is not done: {:?}", game.save.quizzes.get(page)),
        Step::Draft(puzzle, texts) => ui.drafts[puzzle] = texts,
        Step::Commit(puzzle) => sim.request = Some(Request::Commit(puzzle)),
        Step::CarryAt { feet, facing, expect, aim } => {
            player.1.translation = feet;
            player.1.rotation = Quat::from_rotation_y(f32::atan2(-facing.x, -facing.z));
            player.0.velocity = Vec3::ZERO;
            let target = carry::target_for(&world, &sites, &carry, feet, facing, aim);
            assert!(expect.matches(target), "at {feet} facing {facing}, F would do {target:?}, not {expect:?}");
            carry::apply(&mut world, &mut sites, &mut carry, &mut dirty, &mut game, target.expect("checked above"));
            script.wait = 0.3;
        }
        Step::ExpectRefusal(reason) => {
            assert!(ui.refusal.as_deref().is_some_and(|r| r.contains(reason)), "expected a refusal for '{reason}', got {:?}", ui.refusal);
        }
        Step::ExpectNotFound(page) => assert!(!game.save.found.contains(page), "the handbook page '{page}' was written"),
        Step::NotePlate => {
            crate::ui::note_plate(&mut game);
        }
        Step::Calculate(text) => ui.calculator = text,
        Step::ExpectSolved(puzzle) => {
            assert!(game.solved(puzzle), "{} is not solved: {:?}", game.def(puzzle).title, ui.results[puzzle].as_ref().map(|o| &o.detail))
        }
        Step::Windows { notebook, handbook, tutor } => {
            ui.notebook_open = notebook;
            ui.handbook_open = handbook;
            ui.tutor_open = tutor;
        }
        Step::Shot(name) => {
            let path = script.shots.join(format!("{name}.png"));
            commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
            script.wait = 0.4;
        }
        Step::Key(key, down) => {
            if down {
                keys.press(key);
            } else {
                keys.release(key);
            }
        }
        Step::UntilStandingAt { height, within, what } => {
            let y = player.1.translation.y;
            if !(player.0.grounded && (y - height).abs() < 0.01) {
                // Budgets count the avatar's own (capped) time step, so a slow renderer does not fail them.
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "the avatar did not {what}: it is at {} (height {y}, wanted {height})", player.1.translation);
                script.steps.push_front(Step::UntilStandingAt { height, within: left, what });
            }
        }
        Step::UntilNorthOf { z, within, what } => {
            if player.1.translation.z > z {
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "the avatar did not {what}: it is at {}", player.1.translation);
                script.steps.push_front(Step::UntilNorthOf { z, within: left, what });
            }
        }
        Step::UntilFallenIn { within } => {
            if shore.fallen_in == 0 {
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "the avatar did not fall into the channel: it is at {}", player.1.translation);
                script.steps.push_front(Step::UntilFallenIn { within: left });
            }
        }
        Step::ExpectStanding { height, what } => {
            let y = player.1.translation.y;
            assert!(player.0.grounded && (y - height).abs() < 0.01, "the avatar did not {what}: it is at {}", player.1.translation);
        }
        Step::UntilAirborneAbove { height, within } => {
            if player.0.grounded || player.1.translation.y <= height {
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "the avatar did not leave the ground");
                script.steps.push_front(Step::UntilAirborneAbove { height, within: left });
            }
        }
        Step::Start => intro.as_mut().expect("the title menu is up").request_start(),
        Step::UntilPlaying { within } => {
            if *mode.get() != Mode::Playing {
                let left = within - time.delta_secs().min(0.25);
                assert!(left > 0.0, "the game did not take over from the menu");
                script.steps.push_front(Step::UntilPlaying { within: left });
            }
        }
        Step::UntilIntro(at) => {
            let clock = intro.as_ref().and_then(|intro| intro.clock()).expect("the intro is playing");
            if clock < at {
                script.steps.push_front(Step::UntilIntro(at));
            }
        }
        Step::ExpectPlayerView => {
            let wanted = orbit.eye(player.1.translation + Vec3::Y * 1.5);
            let eye = camera.1.translation();
            assert!(eye.distance(wanted) < 0.3, "the camera is at {eye}, the player's view at {wanted}");
        }
        Step::UntilMoved { from, within } => {
            let here = player.1.translation;
            let from = from.unwrap_or(here);
            if here.distance(from) < 1.0 {
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "the avatar did not move from {from}");
                script.steps.push_front(Step::UntilMoved { from: Some(from), within: left });
            }
        }
        Step::ExpectActivity { activity, within } => {
            if avatar.activity != activity {
                let left = within - time.delta_secs().min(1.0 / 20.0);
                assert!(left > 0.0, "the avatar did not show {activity:?}: it shows {:?}", avatar.activity);
                script.steps.push_front(Step::ExpectActivity { activity, within: left });
            }
        }
        Step::Quit => {
            exit.write(AppExit::Success);
        }
    }
}
