//! Scripted runs for automated checks. Development only; enabled by `GAME_SCRIPT`.
//!
//! - `tour` plays chapter 1.1, tutorial first, with the expressions a player would type and writes
//!   screenshots.
//! - `walk` drives the avatar with key presses: up the steps, off the rim, out of the canyon, a jump.
//! - `pose` films the avatar up close: standing, walking, running, jumping, at a console, watching a
//!   test, and its reactions to a miss and a pass.
//! - `intro` photographs the intro behind the title menu, presses Start and checks that the game
//!   takes over: the camera ends on the player's view and the avatar walks.
//!
//! `tour`, `walk` and `pose` press Start first, as a player would.
//! The scripts go through the same requests as the interface (blueprint text, commit, ruler picks,
//! key state), so they exercise the real paths rather than setting results directly. A script that
//! does not see what it expects panics, which fails the run.

use crate::avatar::{Activity, AvatarState};
use crate::carry::{self, Carry, DirtyCells, Target};
use crate::companion::{Companion, station_name};
use crate::intro::Intro;
use crate::island::Sites;
use crate::player::{OrbitCamera, Player};
use crate::ruler::{Ruler, pick_at};
use crate::sim::{Request, Sim};
use crate::state::{Game, Mode, Ui};
use crate::voxel::VoxelWorld;
use bevy::prelude::*;
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
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
    Look { yaw: f32, pitch: f32, distance: f32 },
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
    ExpectGaze { place: String, within: f32 },
    /// Pick a choice in a station's experiment, as a click on it would.
    Guess(usize, usize),
    /// Press the experiment's run button.
    RunDemo(usize),
    /// Wait until a handbook page has been written; fail if it has not happened in time.
    ExpectFound { page: &'static str, within: f32 },
    /// Open the handbook at a page, as its button at the station would.
    OpenPage(&'static str),
    Draft(usize, Vec<String>),
    Commit(usize),
    /// Stand at `feet` facing along `facing` and press F; fail unless F would do what is expected.
    CarryAt { feet: Vec3, facing: Vec3, expect: Expect },
    /// Press the ledge launcher's rating plate's Note button.
    NotePlate,
    /// Type into the calculator.
    Calculate(String),
    Windows { notebook: bool, handbook: bool, tutor: bool },
    Shot(&'static str),
    /// Press or release a key, as the keyboard would.
    Key(KeyCode, bool),
    /// Wait until the avatar stands at this height; fail if it has not happened in time.
    UntilStandingAt { height: f32, within: f32, what: &'static str },
    /// Wait until the avatar is in the air above this height.
    UntilAirborneAbove { height: f32, within: f32 },
    /// Wait until the avatar's animation shows this activity; fail if it has not happened in time.
    ExpectActivity { activity: Activity, within: f32 },
    /// Press Start on the title menu.
    Start,
    /// Wait until the game has taken over from the menu.
    UntilPlaying { within: f32 },
    /// Wait until the intro is this many seconds into its loop.
    UntilIntro(f32),
    /// Check that the camera stands where the player's view puts it.
    ExpectPlayerView,
    /// Wait until the avatar has moved a metre from where it stood when the step began.
    UntilMoved { from: Option<Vec3>, within: f32 },
    Quit,
}

/// What a press of F should do.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Expect {
    Take,
    PutInFrame,
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
        let game = app.world().resource::<Game>();
        let sites = app.world().resource::<Sites>();
        let steps = match name.as_str() {
            "tour" => tour(game, sites),
            "walk" => walk(game, sites),
            "intro" => intro(),
            "pose" => pose(game, sites),
            other => panic!("unknown GAME_SCRIPT '{other}' (known: tour, walk, intro, pose)"),
        };
        let shots = PathBuf::from(std::env::var("GAME_SHOTS").unwrap_or_else(|_| "shots".into()));
        std::fs::create_dir_all(&shots).expect("the screenshot directory can be created");
        app.insert_resource(Script { steps: steps.into(), wait: 2.0, shots }).add_systems(Update, run);
    }
}

fn tour(game: &Game, sites: &Sites) -> Vec<Step> {
    use Step::*;
    let (min, max) = game.layout.ridge_angle_window();
    let angle = ((min + max) / 2.0).to_degrees().round();
    let closed = Windows { notebook: false, handbook: false, tutor: false };
    let mut steps = vec![
        Start,
        UntilPlaying { within: 10.0 },
        Shot("01_start"),
        // Station 0 is behind a close camera: Lumen's beam ends in an arrow at the screen's edge.
        expect_gaze(station_name(game, 0)),
        Look { yaw: -0.8, pitch: 0.15, distance: 3.0 },
        Wait(1.2),
        Shot("01b_lumen_points_off_screen"),
        Watch(Some(Vec3::new(80.0, 20.0, 66.0) + crate::island::LAYOUT_ORIGIN.as_vec3())),
        Look { yaw: -0.5, pitch: 0.75, distance: 80.0 },
        Wait(1.5),
        Shot("02_island"),
        Watch(None),
        // Puzzle 0, the tutorial: measure the rail, a bare number is refused, the reading passes.
        GoToStation(0),
        Look { yaw: 0.35, pitch: 0.5, distance: 22.0 },
        Wait(1.0),
        expect_gaze(station_name(game, 0)),
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
        Windows { notebook: false, handbook: true, tutor: false },
        Wait(0.4),
        Shot("06b_first_page_and_a_locked_one"),
        closed_step(),
        // Puzzle 1: measure the tower, time the drop, work out g.
        GoToStation(1),
        // Away from the solved station, Lumen points ahead to the next one.
        expect_gaze(station_name(game, 1)),
        Look { yaw: 0.9, pitch: 0.35, distance: 26.0 },
        Wait(1.0),
        Station(Some(1)),
        // Lumen points at the experiment until it has been watched.
        expect_gaze("Drop tower: release point"),
        Wait(0.6),
        Shot("07a_try_it_first"),
        RulerPin(0),
        RulerPin(1),
        Wait(0.3),
        Shot("07_ruler_on_tower"),
        RulerNote(1),
        RulerOff,
        Guess(1, 1),
        RunDemo(1),
        Wait(1.6),
        Shot("08_strobe_drop"),
        WaitIdle,
        ExpectFound { page: "free_fall", within: 2.0 },
        Wait(0.6),
        Shot("08b_strobe_seen"),
        Windows { notebook: true, handbook: false, tutor: false },
        Draft(1, vec!["2*h1/t1^2".into()]),
        Wait(0.3),
        Shot("09_gravity_blueprint"),
        closed_step(),
        Commit(1),
        Wait(1.0),
        Shot("10_mast_test"),
        WaitIdle,
        Wait(3.0),
        Shot("11_gravity_result"),
        // Puzzle 2: a refused entry, a miss, then the calculated shot.
        GoToStation(2),
        Look { yaw: -1.05, pitch: 0.45, distance: 30.0 },
        Wait(1.0),
        Station(Some(2)),
        // A wrong guess, on purpose: the dropped crate first.
        Guess(2, 0),
        RunDemo(2),
        Wait(1.5),
        Shot("11a_drop_and_fire"),
        WaitIdle,
        ExpectFound { page: "independence", within: 2.0 },
        Wait(0.5),
        Shot("11b_drop_and_fire_landed"),
        Wait(3.0),
        RulerPin(4),
        RulerPin(5),
        Wait(0.3),
        Shot("12_ruler_across_canyon"),
        RulerNote(0),
        RulerNote(1),
        RulerOff,
        // The rating plate, then the height the launcher needs above the ledge, in the calculator.
        NotePlate,
        Windows { notebook: true, handbook: false, tutor: false },
        Calculate("rise = 0.5*g*(d2/v1)^2 - h2".into()),
        Wait(0.5),
        Shot("13_calculator_rise"),
        closed_step(),
        Station(None),
    ];
    // One crate first, on purpose: it falls short. Then the rest of the worked-out stack.
    let stack = game.layout.canyon_answer().expect("the canyon has one answer");
    let pile_west = sites.crate_pile.iter().map(|c| c.x).min().expect("there is a pile") as f32;
    let pile_row = sites.crate_pile.iter().map(|c| c.z).min().expect("there is a pile") as f32;
    let ledge = sites.canyon_well.y as f32;
    let at_pile = Vec3::new(pile_west - 0.6, ledge, pile_row + 0.5);
    let at_frame = sites.canyon_well.as_vec3() + Vec3::new(-0.9, 0.0, 0.5);
    let fetch = |steps: &mut Vec<Step>, shots: bool| {
        steps.push(CarryAt { feet: at_pile, facing: Vec3::X, expect: Expect::Take });
        if shots {
            steps.extend([Look { yaw: 2.3, pitch: 0.35, distance: 7.0 }, Wait(0.8), Shot("14a_carrying")]);
        }
        steps.push(CarryAt { feet: at_frame, facing: Vec3::X, expect: Expect::PutInFrame });
    };
    fetch(&mut steps, true);
    steps.extend([
        GoToStation(2),
        Station(Some(2)),
        Commit(2),
        WaitIdle,
        Wait(2.0),
        Shot("14_canyon_miss"),
        Station(None),
    ]);
    for _ in 1..stack {
        fetch(&mut steps, false);
    }
    steps.extend([
        Look { yaw: 2.5, pitch: 0.12, distance: 17.0 },
        Wait(1.0),
        Shot("14b_launcher_raised"),
        GoToStation(2),
        Look { yaw: -1.4, pitch: 0.45, distance: 30.0 },
        Station(Some(2)),
        Commit(2),
        Wait(1.2),
        Shot("15_canyon_flight"),
        WaitIdle,
        Wait(2.5),
        Shot("16_canyon_result"),
        // Puzzle 3: 45 degrees strikes the ridge; an angle inside the window clears it.
        GoToStation(3),
        Look { yaw: -1.15, pitch: 0.4, distance: 34.0 },
        Wait(1.0),
        Station(Some(3)),
        Guess(3, 2),
        RunDemo(3),
        Wait(1.4),
        Shot("16a_two_angles"),
        WaitIdle,
        ExpectFound { page: "range", within: 2.0 },
        Wait(0.5),
        Shot("16b_two_angles_landed"),
        Wait(3.0),
        RulerPin(6),
        RulerPin(9),
        RulerNote(0),
        RulerOff,
        Draft(3, vec!["45 deg".into(), "sqrt(g*d3)".into()]),
        Commit(3),
        WaitIdle,
        Wait(2.0),
        Shot("17_ridge_struck"),
        Draft(3, vec![format!("{angle} deg"), format!("sqrt(g*d3/sin(2*{angle} deg))")]),
        // The launcher shows the blueprint before anything is committed: the arm tilts, the gauge fills.
        Watch(Some(sites.ridge_launch.origin + sites.ridge_launch.direction * 1.5 + Vec3::Y * 1.2)),
        Look { yaw: -0.75, pitch: 0.2, distance: 9.5 },
        Wait(1.5),
        Shot("17b_launcher_shows_the_blueprint"),
        Watch(None),
        Look { yaw: -1.15, pitch: 0.4, distance: 34.0 },
        Wait(0.5),
        Commit(3),
        Wait(1.8),
        Shot("18_ridge_flight"),
        WaitIdle,
        Wait(3.0),
        Shot("19_ridge_result"),
        Station(None),
        expect_gaze("The shipyard floor"),
        GoTo(sites.shipyard + Vec3::new(-11.0, 0.0, 9.0)),
        Look { yaw: -0.8, pitch: 0.3, distance: 16.0 },
        Wait(3.5),
        Shot("20_frame_installed"),
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
    let ledge = sites.canyon_launch.origin;
    let east = -std::f32::consts::FRAC_PI_2;
    let south = std::f32::consts::PI;
    vec![
        Start,
        UntilPlaying { within: 10.0 },
        // Start on the grass west of the steps and walk east: up the steps onto the outcrop.
        GoTo(Vec3::new(ledge.x - 20.0 - game.layout.ledge_h as f32, ground, ledge.z)),
        Look { yaw: east, pitch: 0.5, distance: 16.0 },
        Wait(0.5),
        Key(KeyCode::KeyW, true),
        expect_activity(Activity::Walk),
        UntilStandingAt { height: ledge.y, within: 12.0, what: "climb the steps to the top of the outcrop" },
        Shot("walk_01_on_the_ledge"),
        // Keep walking: off the rim and down to the canyon floor.
        UntilStandingAt { height: ground - game.layout.canyon_depth as f32, within: 12.0, what: "fall to the canyon floor" },
        Key(KeyCode::KeyW, false),
        Shot("walk_02_in_the_canyon"),
        // Turn south and walk out up the stepped end of the canyon.
        Look { yaw: south, pitch: 0.5, distance: 16.0 },
        Wait(0.3),
        Key(KeyCode::KeyW, true),
        UntilStandingAt { height: ground, within: 20.0, what: "walk out of the canyon" },
        Key(KeyCode::KeyW, false),
        Wait(0.3),
        // A jump leaves the ground and comes back to it.
        Key(KeyCode::Space, true),
        UntilAirborneAbove { height: ground + 0.5, within: 2.0 },
        expect_activity(Activity::Air),
        Key(KeyCode::Space, false),
        UntilStandingAt { height: ground, within: 4.0, what: "land after the jump" },
        expect_activity(Activity::Idle),
        Shot("walk_03_out_again"),
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
        Key(KeyCode::KeyW, true),
        UntilMoved { from: None, within: 3.0 },
        Key(KeyCode::KeyW, false),
        Quit,
    ]
}

fn pose(game: &Game, sites: &Sites) -> Vec<Step> {
    use Step::*;
    let pile_west = sites.crate_pile.iter().map(|c| c.x).min().expect("there is a pile") as f32;
    let pile_row = sites.crate_pile.iter().map(|c| c.z).min().expect("there is a pile") as f32;
    let at_pile = Vec3::new(pile_west - 0.6, sites.canyon_well.y as f32, pile_row + 0.5);
    let at_frame = sites.canyon_well.as_vec3() + Vec3::new(-0.9, 0.0, 0.5);
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
        // A crate from the ledge's pile, carried.
        CarryAt { feet: at_pile, facing: Vec3::X, expect: Expect::Take },
        Look { yaw: 2.3, pitch: 0.25, distance: 4.5 },
        Wait(0.8),
        Shot("pose_09_carrying"),
        Key(KeyCode::KeyS, true),
        Wait(0.45),
        Shot("pose_10_carrying_walk"),
        Key(KeyCode::KeyS, false),
        // Hands free again before the console: the crate goes into the jack frame.
        CarryAt { feet: at_frame, facing: Vec3::X, expect: Expect::PutInFrame },
        Wait(0.5),
        // At the rail's console: working the panel, watching the cart, a miss and then a pass.
        GoToStation(0),
        at_console(),
        Station(Some(0)),
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
    (mut intro, mode, avatar, mut carry, mut dirty): (Option<ResMut<Intro>>, Res<State<Mode>>, Single<&AvatarState>, ResMut<Carry>, ResMut<DirtyCells>),
) {
    if script.wait > 0.0 {
        script.wait -= time.delta_secs();
        return;
    }
    let Some(step) = script.steps.pop_front() else { return };
    match step {
        Step::Wait(seconds) => script.wait = seconds,
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
        Step::Look { yaw, pitch, distance } => {
            orbit.yaw = yaw;
            orbit.pitch = pitch;
            orbit.distance = distance;
        }
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
            game.save.notebook.add_numbered(prefix, "measured by the script", Quantity::metres(value as f64), "ruler");
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
        Step::Draft(puzzle, texts) => ui.drafts[puzzle] = texts,
        Step::Commit(puzzle) => sim.request = Some(Request::Commit(puzzle)),
        Step::CarryAt { feet, facing, expect } => {
            player.1.translation = feet;
            player.1.rotation = Quat::from_rotation_y(f32::atan2(-facing.x, -facing.z));
            player.0.velocity = Vec3::ZERO;
            let target = carry::target_for(&world, &sites, &carry, feet, facing);
            let ok = matches!((expect, target), (Expect::Take, Some(Target::Take(_))) | (Expect::PutInFrame, Some(Target::PutInFrame)));
            assert!(ok, "at {feet} facing {facing}, F would do {target:?}, not {expect:?}");
            carry::apply(&mut world, &mut sites, &mut carry, &mut dirty, &mut game, target.expect("checked above"));
            script.wait = 0.3;
        }
        Step::NotePlate => {
            crate::ui::note_plate(&mut game);
        }
        Step::Calculate(text) => ui.calculator = text,
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
