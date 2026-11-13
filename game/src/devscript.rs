//! Scripted runs for automated checks. Development only; enabled by `GAME_SCRIPT`.
//!
//! - `tour` plays chapter 1.1, tutorial first, with the expressions a player would type and writes
//!   screenshots.
//! - `walk` drives the avatar with key presses: up the steps, off the rim, out of the canyon, a jump.
//!
//! The scripts go through the same requests as the interface (blueprint text, commit, ruler picks,
//! key state), so they exercise the real paths rather than setting results directly. A script that
//! does not see what it expects panics, which fails the run.

use crate::island::Sites;
use crate::player::{OrbitCamera, Player};
use crate::ruler::{Ruler, pick_at};
use crate::sim::{Request, Sim};
use crate::state::{Game, Ui};
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
    RulerOff,
    Release,
    Draft(usize, Vec<String>),
    Commit(usize),
    Windows { notebook: bool, handbook: bool, tutor: bool },
    Shot(&'static str),
    /// Press or release a key, as the keyboard would.
    Key(KeyCode, bool),
    /// Wait until the avatar stands at this height; fail if it has not happened in time.
    UntilStandingAt { height: f32, within: f32, what: &'static str },
    /// Wait until the avatar is in the air above this height.
    UntilAirborneAbove { height: f32, within: f32 },
    Quit,
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
            other => panic!("unknown GAME_SCRIPT '{other}' (known: tour, walk)"),
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
    vec![
        Shot("01_start"),
        Watch(Some(Vec3::new(80.0, 20.0, 66.0))),
        Look { yaw: -0.5, pitch: 0.75, distance: 80.0 },
        Wait(1.5),
        Shot("02_island"),
        Watch(None),
        // Puzzle 0, the tutorial: measure the rail, a bare number is refused, the reading passes.
        GoToStation(0),
        Look { yaw: 0.35, pitch: 0.5, distance: 22.0 },
        Wait(1.0),
        Station(Some(0)),
        RulerPin(10),
        RulerPin(11),
        Wait(0.3),
        Shot("03_ruler_along_rail"),
        RulerNote(0),
        RulerOff,
        Draft(0, vec![game.layout.rail_target.to_string()]),
        Commit(0),
        Wait(0.3),
        Shot("04_bare_number_refused"),
        Draft(0, vec!["d1".into()]),
        Commit(0),
        Wait(1.5),
        Shot("05_rail_run"),
        WaitIdle,
        Wait(3.0),
        Shot("06_rail_result"),
        // Puzzle 1: measure the tower, time the drop, work out g.
        GoToStation(1),
        Look { yaw: 0.9, pitch: 0.35, distance: 26.0 },
        Wait(1.0),
        Station(Some(1)),
        RulerPin(0),
        RulerPin(1),
        Wait(0.3),
        Shot("07_ruler_on_tower"),
        RulerNote(1),
        RulerOff,
        Release,
        Wait(0.6),
        Shot("08_tower_drop"),
        WaitIdle,
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
        RulerPin(4),
        RulerPin(5),
        Wait(0.3),
        Shot("12_ruler_across_canyon"),
        RulerNote(0),
        RulerNote(1),
        RulerOff,
        Draft(2, vec!["sqrt(2*h2/g)".into()]),
        Commit(2),
        Wait(0.3),
        Shot("13_wrong_dimension_refused"),
        Draft(2, vec!["9 m/s".into()]),
        Commit(2),
        WaitIdle,
        Wait(2.0),
        Shot("14_canyon_miss"),
        Draft(2, vec!["d2/sqrt(2*h2/g)".into()]),
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
        Commit(3),
        Wait(1.8),
        Shot("18_ridge_flight"),
        WaitIdle,
        Wait(3.0),
        Shot("19_ridge_result"),
        Station(None),
        GoTo(sites.shipyard + Vec3::new(-11.0, 0.0, 9.0)),
        Look { yaw: -0.8, pitch: 0.3, distance: 16.0 },
        Wait(3.5),
        Shot("20_frame_installed"),
        Windows { notebook: true, handbook: true, tutor: true },
        Wait(0.5),
        Shot("21_notebook_handbook_lumen"),
        closed,
        Quit,
    ]
}

fn walk(game: &Game, sites: &Sites) -> Vec<Step> {
    use Step::*;
    let ground = crate::island::GROUND as f32;
    let ledge = sites.canyon_launch.origin;
    let east = -std::f32::consts::FRAC_PI_2;
    let south = std::f32::consts::PI;
    vec![
        // Start on the grass west of the steps and walk east: up the steps onto the outcrop.
        GoTo(Vec3::new(ledge.x - 20.0 - game.layout.ledge_h as f32, ground, ledge.z)),
        Look { yaw: east, pitch: 0.5, distance: 16.0 },
        Wait(0.5),
        Key(KeyCode::KeyW, true),
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
        Key(KeyCode::Space, false),
        UntilStandingAt { height: ground, within: 4.0, what: "land after the jump" },
        Shot("walk_03_out_again"),
        Quit,
    ]
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
    world: Res<VoxelWorld>,
    sites: Res<Sites>,
    mut player: Single<(&mut Player, &mut Transform), Without<Camera3d>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut exit: MessageWriter<AppExit>,
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
        Step::RulerOff => {
            if ruler.active {
                ruler.toggle();
            }
        }
        Step::Release => sim.request = Some(Request::ReleaseTowerMass),
        Step::Draft(puzzle, texts) => ui.drafts[puzzle] = texts,
        Step::Commit(puzzle) => sim.request = Some(Request::Commit(puzzle)),
        Step::Windows { notebook, handbook, tutor } => {
            ui.show_notebook = notebook;
            ui.show_handbook = handbook;
            ui.show_tutor = tutor;
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
        Step::Quit => {
            exit.write(AppExit::Success);
        }
    }
}
