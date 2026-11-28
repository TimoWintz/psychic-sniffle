//! The beam: chapter 1.1's side-machine, off the road north of the railway (see
//! `physics_core::beam`, and "Side-machines" in the game design).
//!
//! A long beam of the builders' stone on a pivot post. The machine sets its own blocks on a seat of
//! the west arm; the player carries blocks from its pile onto the seats (see `carry`) and releases
//! the beam at its console, which costs nothing. The beam swings until an end rests on its stop, or
//! holds level for a moment. Each time it holds, a stud on the post lights and the machine sets the
//! next load; after the last, it wakes: its quartz and its lamp light, the path of quartz down to the
//! road lights from the beam to the road, and the handbook gains its page. There is no letterbox and
//! no camera move: it happens while the player watches.
//!
//! The beam and every block on it are entities: the beam turns as one, with its blocks. Its pile's
//! blocks are blocks of the world, like the ledge's (see `carry`).

use crate::carry::Carry;
use crate::island::{BEAM_AXLE, BEAM_THICK, BeamSite, Sites};
use crate::player::{OrbitCamera, Player};
use crate::ruler::Ruler;
use crate::state::{BuildWorld, Game, Mode, Ui, WorldEntity, not_paused};
use crate::voxel::VoxelWorld;
use crate::waking::{Changes, MachineTrim, light, stone};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use physics_core::beam::{Arm, CASES, Load, MOST_LOAD, MOST_ON_A_SEAT, SEATS, Swing, cases, current, release};
use physics_core::content::Found;
use physics_core::materials::Material;

/// The beam's id in the chapter and the save.
pub const BEAM_ID: &str = "1.1.s1";

/// How close the avatar must be to use the console.
const REACH: f32 = 4.5;
/// The size of a block on the beam (the size of one carried), and how long the beam is: its ends
/// reach half a metre past the last seats.
const BLOCK: f32 = 0.8;
const LENGTH: f32 = 2.0 * SEATS as f32 + 1.4;
/// How far the beam turns to rest an end on its stop.
pub fn most_tilt() -> f32 {
    ((BEAM_AXLE - BEAM_THICK / 2.0) / (LENGTH / 2.0)).asin()
}

/// A release, from the moment the pin comes out: the beam falls to its stop (or wobbles level), rests
/// there while the result is read, and comes back level, pinned again.
const FALL: f32 = 0.9;
const SETTLE: f32 = 1.4;
const REST: f32 = 2.8;
const BACK: f32 = 3.6;
/// The wake: how fast the light runs down the path to the road.
const PATH_SPEED: f32 = 6.0;

/// The beam's state that isn't in the save.
#[derive(Resource)]
pub struct Beam {
    /// The loads the machine sets in this save, in order.
    pub cases: [Load; CASES],
    /// The release playing out, if one is.
    pub swinging: Option<Swinging>,
    /// What the last release did, shown under the button.
    pub last: Option<Swing>,
    /// Releases this session, so Lumen notices each one.
    pub releases: u32,
    /// Seconds into the wake, while it plays.
    pub waking: Option<f32>,
    frame_shown: bool,
    blocks_shown: Option<(physics_core::beam::Placement, Load)>,
    looks_lit: Option<(u32, bool)>,
}

impl Beam {
    pub fn new(seed: u64) -> Self {
        Beam { cases: cases(seed), swinging: None, last: None, releases: 0, waking: None, frame_shown: false, blocks_shown: None, looks_lit: None }
    }
}

/// A release as it plays.
#[derive(Clone, Copy, Debug)]
pub struct Swinging {
    pub swing: Swing,
    /// The load it was released against: the beam shows it until it is pinned again.
    pub load: Load,
    pub t: f32,
}

impl Beam {
    /// Whether the beam is actively swinging before settling on its result.
    pub fn animating(&self) -> bool {
        self.swinging.is_some_and(|s| s.t < SETTLE)
    }

    /// The load on the beam now: the one a release is playing against, or the one set next.
    pub fn load(&self, game: &Game) -> Load {
        self.swinging.map_or_else(|| current(&self.cases, held(game)), |s| s.load)
    }
}

/// How many of the cases have held, in the save.
pub fn held(game: &Game) -> u32 {
    game.save.side(BEAM_ID).held
}

pub fn woken(game: &Game) -> bool {
    game.save.side_woken(BEAM_ID)
}

/// The beam's tilt `t` seconds into a release, as a share of the most it turns: +1 with the far end
/// down on its stop, −1 with the load's end down. Level, it shivers and settles.
pub fn tilt_at(swing: Swing, t: f32) -> f32 {
    let target = swing.tilt();
    if target == 0.0 {
        let left = (1.0 - t / SETTLE).max(0.0);
        return 0.12 * (t * 14.0).sin() * left * left;
    }
    if t < FALL {
        // It tips slowly at first, then faster, as a beam does.
        target * (t / FALL).powi(2)
    } else if t < SETTLE {
        // A small bounce off the stop.
        target * (1.0 - 0.1 * (std::f32::consts::PI * (t - FALL) / (SETTLE - FALL)).sin())
    } else if t < REST {
        target
    } else {
        let back = ((t - REST) / (BACK - REST)).clamp(0.0, 1.0);
        target * (1.0 - back * back * (3.0 - 2.0 * back))
    }
}

/// Put the beam's quartz path to the road lit or dark at once (the island is built with it lit).
pub fn settle(world: &mut VoxelWorld, site: &BeamSite, woken: bool) -> Changes {
    let mut changes = Changes::default();
    light(world, site.path.cells.iter().map(|&(c, _)| c), woken, &mut changes);
    changes
}

/// The beam, turning about its axle, with everything on it.
#[derive(Component)]
struct Body;

/// One of the player's blocks on a seat: its arm, seat and place in the stack (0 at the bottom).
#[derive(Component)]
struct OnSeat(Arm, u32, u32);

/// One of the machine's own blocks, its place in the load's stack.
#[derive(Component)]
struct LoadBlock(u32);

/// The quartz that lights as the beam wakes, and the studs on its post, one for each case that has
/// held.
#[derive(Resource)]
struct Looks {
    wake: Handle<StandardMaterial>,
    studs: [Handle<StandardMaterial>; CASES],
}

pub struct BeamPlugin;

impl Plugin for BeamPlugin {
    fn build(&self, app: &mut App) {
        let seed = app.world().resource::<Game>().save.seed;
        app.insert_resource(Beam::new(seed))
            .add_systems(BuildWorld, spawn)
            .add_systems(
                Update,
                (
                    (proximity, frame).chain().run_if(in_state(Mode::Playing)).run_if(not_paused),
                    (play.run_if(in_state(Mode::Playing)).run_if(not_paused), pose, show_blocks, light_looks).chain(),
                ),
            )
            .add_systems(EguiPrimaryContextPass, panel.after(crate::ui::draw).run_if(in_state(Mode::Playing)));
    }
}

fn spawn(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<Sites>, game: Res<Game>) {
    commands.insert_resource(Beam::new(game.save.seed));
    let site = &sites.beam;
    let stone = stone(&mut materials);
    let awake = woken(&game);
    let looks =
        Looks { wake: materials.add(MachineTrim::look(awake)), studs: std::array::from_fn(|i| materials.add(MachineTrim::look((i as u32) < held(&game)))) };
    let handle = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.56, 0.10), emissive: LinearRgba::rgb(0.5, 0.2, 0.02), ..default() });
    let c = Material::Limestone.properties().colour;
    let block_stone = materials.add(StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), perceptual_roughness: 0.9, ..default() });

    // The beam: a long bar, a quartz stud on its south face at every seat, a hub round the axle.
    let bar = meshes.add(Cuboid::new(LENGTH, BEAM_THICK, 0.8));
    let stud = meshes.add(Cuboid::new(0.12, 0.12, 0.04));
    let hub = meshes.add(Cylinder::new(0.22, 1.0));
    let cube = meshes.add(Cuboid::from_length(BLOCK));
    let grip = meshes.add(Cuboid::new(0.5, 0.07, 0.12));
    commands.spawn((WorldEntity, Body, Transform::from_translation(site.pivot), Visibility::default())).with_children(|b| {
        b.spawn((Mesh3d(bar), MeshMaterial3d(stone.clone()), Transform::default()));
        b.spawn((Mesh3d(hub), MeshMaterial3d(looks.wake.clone()), Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2))));
        for arm in [Arm::Load, Arm::Far] {
            for seat in 1..=SEATS {
                let x = site.seat(arm, seat).x - site.pivot.x;
                b.spawn((Mesh3d(stud.clone()), MeshMaterial3d(looks.wake.clone()), Transform::from_xyz(x, 0.0, 0.41)));
                for level in 0..MOST_ON_A_SEAT {
                    b.spawn((OnSeat(arm, seat, level), Mesh3d(cube.clone()), MeshMaterial3d(block_stone.clone()), Transform::default(), Visibility::Hidden))
                        .with_children(|block| {
                            block.spawn((Mesh3d(grip.clone()), MeshMaterial3d(handle.clone()), Transform::from_xyz(0.0, BLOCK / 2.0 + 0.02, 0.0)));
                        });
                }
            }
        }
        for level in 0..MOST_LOAD {
            b.spawn((LoadBlock(level), Mesh3d(cube.clone()), MeshMaterial3d(block_stone.clone()), Transform::default(), Visibility::Hidden));
        }
    });

    // The post's cheeks either side of the beam, and a stud on its south face for each case.
    let post = site.pivot.with_y(site.pivot.y - BEAM_AXLE);
    let cheek = meshes.add(Cuboid::new(0.7, 0.6, 0.12));
    for side in [-1.0, 1.0] {
        commands.spawn((
            WorldEntity,
            Mesh3d(cheek.clone()),
            MeshMaterial3d(stone.clone()),
            Transform::from_translation(site.pivot + Vec3::new(0.0, -0.05, side * 0.48)),
        ));
    }
    let case_stud = meshes.add(Cuboid::new(0.22, 0.16, 0.06));
    for (i, look) in looks.studs.iter().enumerate() {
        let at = post + Vec3::new(-0.3 + 0.3 * i as f32, 0.55, 0.52);
        commands.spawn((WorldEntity, Mesh3d(case_stud.clone()), MeshMaterial3d(look.clone()), Transform::from_translation(at)));
    }

    // The console: a pillar of their stone with a sloping top and a quartz face, which answers from
    // the start, as the supply rail's does.
    let foot = site.console.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    let slope = Quat::from_rotation_x(-0.4);
    let slab = Transform::from_translation(foot + Vec3::Y * 1.05).with_rotation(slope);
    let face = materials.add(MachineTrim::look(true));
    commands.spawn((
        WorldEntity,
        Mesh3d(meshes.add(Cuboid::new(0.6, 1.0, 0.6))),
        MeshMaterial3d(stone.clone()),
        Transform::from_translation(foot + Vec3::Y * 0.5),
    ));
    commands.spawn((WorldEntity, Mesh3d(meshes.add(Cuboid::new(0.86, 0.14, 0.7))), MeshMaterial3d(stone.clone()), slab));
    commands.spawn((
        WorldEntity,
        Mesh3d(meshes.add(Cuboid::new(0.62, 0.05, 0.46))),
        MeshMaterial3d(face),
        slab.with_translation(slab.translation + slab.rotation * Vec3::Y * 0.08),
    ));
    // Its lamp, dark until the beam wakes.
    commands.spawn((
        WorldEntity,
        Mesh3d(meshes.add(Cuboid::new(0.22, 2.3, 0.22))),
        MeshMaterial3d(stone.clone()),
        Transform::from_translation(site.lamp + Vec3::Y * 1.15),
    ));
    commands.spawn((
        WorldEntity,
        Mesh3d(meshes.add(Cuboid::new(0.3, 0.38, 0.3))),
        MeshMaterial3d(looks.wake.clone()),
        Transform::from_translation(site.lamp + Vec3::Y * 2.49),
    ));
    commands.spawn((
        WorldEntity,
        Mesh3d(meshes.add(Cuboid::new(0.46, 0.1, 0.46))),
        MeshMaterial3d(stone),
        Transform::from_translation(site.lamp + Vec3::Y * 2.73),
    ));
    commands.insert_resource(looks);
}

/// Whether the avatar at `feet` is near enough to use the console.
pub fn near_console(site: &BeamSite, feet: Vec3) -> bool {
    (site.console.as_vec3() + Vec3::splat(0.5)).distance(feet + Vec3::Y * 0.5) <= REACH
}

/// E at the console opens its panel and closes it again; walking away closes it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn proximity(
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    sites: Res<Sites>,
    sim: Res<crate::sim::Sim>,
    beam: Res<Beam>,
    waking: Res<crate::waking::Waking>,
    mut ui: ResMut<Ui>,
    player: Single<&Transform, With<Player>>,
) {
    let near = near_console(&sites.beam, player.translation);
    if ui.near_beam != near {
        ui.near_beam = near;
    }
    if ui.beam_open && !near {
        ui.beam_open = false;
    }
    if near && !wants.wants_any_keyboard_input() && !crate::ui::hud_hidden(&waking, &sim, &beam) && keys.just_pressed(KeyCode::KeyE) {
        ui.beam_open = !ui.beam_open;
    }
}

/// While the console is open the camera shows the beam from the north, where the ground is open and
/// nothing stands between, and glides back after.
pub(crate) fn frame(ui: Res<Ui>, sites: Res<Sites>, mut orbit: ResMut<OrbitCamera>, mut ruler: ResMut<Ruler>, mut beam: ResMut<Beam>) {
    if ui.beam_open == beam.frame_shown {
        return;
    }
    beam.frame_shown = ui.beam_open;
    if ui.beam_open {
        let (min, max) = view(&sites.beam);
        orbit.frame_side_view(min, max, Vec3::NEG_X, true);
    } else {
        ruler.put_away();
        if ui.station.is_none() {
            orbit.ease_home();
        }
    }
}

/// The box the camera shows while the console is open: the beam swung either way, with its tallest
/// stacks.
pub fn view(site: &BeamSite) -> (Vec3, Vec3) {
    let reach = LENGTH / 2.0 + 0.5;
    (site.pivot + Vec3::new(-reach, -BEAM_AXLE, -0.5), site.pivot + Vec3::new(reach, 3.5, 0.5))
}

/// Play a release, count what it showed, and wake the beam after the last case.
#[allow(clippy::too_many_arguments)]
fn play(
    time: Res<Time>,
    sites: Res<Sites>,
    mut beam: ResMut<Beam>,
    mut game: ResMut<Game>,
    mut ui: ResMut<Ui>,
    mut carry: ResMut<Carry>,
    mut world: ResMut<VoxelWorld>,
    mut dirty: ResMut<crate::carry::DirtyCells>,
) {
    let dt = time.delta_secs().min(0.25);
    if let Some(mut swinging) = beam.swinging {
        let before = swinging.t;
        swinging.t += dt;
        if before < SETTLE && swinging.t >= SETTLE {
            // It has come to rest: now it counts.
            beam.last = Some(swinging.swing);
            beam.releases += 1;
            let progress = game.save.sides.entry(BEAM_ID.to_string()).or_default();
            if swinging.swing.holds() && (progress.held as usize) < CASES {
                progress.held += 1;
            }
            game.persist();
        }
        if swinging.t >= BACK {
            beam.swinging = None;
            carry.beam_swinging = false;
            if held(&game) as usize >= CASES && !woken(&game) {
                wake(&mut game, &mut ui);
                beam.waking = Some(0.0);
            }
        } else {
            beam.swinging = Some(swinging);
        }
    }
    if let Some(t) = beam.waking {
        let t = t + dt;
        let path = &sites.beam.path;
        let mut changes = Changes::default();
        light(&mut world, path.cells.iter().filter(|(_, s)| *s <= t * PATH_SPEED).map(|&(c, _)| c), true, &mut changes);
        dirty.0.extend(changes.cells);
        beam.waking = (t * PATH_SPEED <= path.length() + 1.0).then_some(t);
    }
}

/// The beam has woken: keep it, and write its page.
fn wake(game: &mut Game, ui: &mut Ui) {
    game.save.sides.entry(BEAM_ID.to_string()).or_default().woken = true;
    let pages: Vec<String> = game.chapter.concepts.iter().filter(|c| c.found == Found::Side(BEAM_ID.to_string())).map(|c| c.id.clone()).collect();
    for page in pages {
        if game.save.find(&page) {
            ui.unread_pages.push(page);
        }
    }
    game.persist();
}

/// Release the beam against the load on it now. Also used by the scripted runs.
pub fn release_now(beam: &mut Beam, game: &Game, carry: &mut Carry) {
    if beam.swinging.is_some() || carry.holding.is_some() {
        return;
    }
    let load = beam.load(game);
    beam.swinging = Some(Swinging { swing: release(load, &carry.beam), load, t: 0.0 });
    carry.beam_swinging = true;
}

/// Turn the beam to its tilt.
fn pose(beam: Res<Beam>, mut body: Single<&mut Transform, With<Body>>) {
    let tilt = beam.swinging.map_or(0.0, |s| tilt_at(s.swing, s.t));
    // The far end is east: turning about +Z raises it, so it goes down the other way.
    body.rotation = Quat::from_rotation_z(-tilt * most_tilt());
}

/// Show the blocks on each seat, and the machine's load on its seat.
fn show_blocks(
    mut beam: ResMut<Beam>,
    game: Res<Game>,
    carry: Res<Carry>,
    sites: Res<Sites>,
    mut seats: Query<(&OnSeat, &mut Transform, &mut Visibility), Without<LoadBlock>>,
    mut loads: Query<(&LoadBlock, &mut Transform, &mut Visibility), Without<OnSeat>>,
) {
    let load = beam.load(&game);
    if beam.blocks_shown == Some((carry.beam, load)) {
        return;
    }
    beam.blocks_shown = Some((carry.beam, load));
    let site = &sites.beam;
    let on = |arm: Arm, seat: u32, level: u32| site.seat(arm, seat) - site.pivot + Vec3::Y * (BLOCK * (level as f32 + 0.5));
    for (block, mut transform, mut visibility) in &mut seats {
        let OnSeat(arm, seat, level) = *block;
        // On the load's seat the machine's blocks come first, and the player's stand on them.
        let under = if arm == Arm::Load && seat == load.seat { load.blocks } else { 0 };
        transform.translation = on(arm, seat, level + under);
        visibility.set_if_neq(if level < carry.beam.get(arm, seat) { Visibility::Inherited } else { Visibility::Hidden });
    }
    for (block, mut transform, mut visibility) in &mut loads {
        transform.translation = on(Arm::Load, load.seat, block.0);
        visibility.set_if_neq(if block.0 < load.blocks { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// Light a stud for each case that has held, and the beam's quartz and lamp once it has woken.
fn light_looks(game: Res<Game>, mut beam: ResMut<Beam>, looks: Res<Looks>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let now = (held(&game), woken(&game));
    if beam.looks_lit == Some(now) {
        return;
    }
    if let Some(mut m) = materials.get_mut(&looks.wake) {
        *m = MachineTrim::look(now.1);
    }
    for (i, handle) in looks.studs.iter().enumerate() {
        if let Some(mut m) = materials.get_mut(handle) {
            *m = MachineTrim::look((i as u32) < now.0);
        }
    }
    beam.looks_lit = Some(now);
}

/// The console's panel.
pub(crate) fn panel(
    mut contexts: EguiContexts,
    game: Res<Game>,
    sim: Res<crate::sim::Sim>,
    waking: Res<crate::waking::Waking>,
    mut ui: ResMut<Ui>,
    mut beam: ResMut<Beam>,
    mut carry: ResMut<Carry>,
) -> Result {
    if !ui.beam_open || ui.paused || crate::ui::hud_hidden(&waking, &sim, &beam) {
        return Ok(());
    }
    let ctx = contexts.ctx_mut()?.clone();
    let Some(def) = game.chapter.side(BEAM_ID).cloned() else { return Ok(()) };
    use crate::ui::{DIM, GOOD, LUMEN, WARN};
    use egui::RichText;
    if let Some(resp) = egui::Window::new(format!("Console: {}", def.title))
        .id(egui::Id::new("beam console"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
        .show(&ctx, |panel| {
            panel.set_width(400.0);
            panel.horizontal(|panel| {
                if panel.button(RichText::new("Briefing").color(LUMEN)).on_hover_text("Talk through this console with Lumen").clicked() {
                    ui.request_briefing = true;
                }
                panel.with_layout(egui::Layout::right_to_left(egui::Align::Center), |panel| {
                    if panel.button("Leave (E)").clicked() {
                        ui.beam_open = false;
                    }
                });
            });
            panel.separator();
            panel.label(RichText::new(&def.task).strong());
            panel.add_space(6.0);
            let done = held(&game).min(CASES as u32);
            let marks: String = (0..CASES as u32).map(|i| if i < done { "■ " } else { "□ " }).collect();
            if woken(&game) {
                panel.label(RichText::new(format!("{marks} All three loads have held, and the beam is awake.")).color(GOOD));
            } else {
                panel.label(format!("{marks} Loads held: {done} of {CASES}"));
            }
            panel.add_space(4.0);
            panel.horizontal(|panel| {
                let free = beam.swinging.is_none() && carry.holding.is_none();
                if panel.add_enabled(free, egui::Button::new(RichText::new("Release the beam").strong())).clicked() {
                    release_now(&mut beam, &game, &mut carry);
                }
                panel.label(RichText::new("Releasing costs nothing.").small().color(DIM));
            });
            if carry.holding.is_some() {
                panel.label(RichText::new("Put down what you're carrying first.").small().color(WARN));
            }
            if let Some(swing) = beam.last {
                panel.colored_label(if swing.holds() { GOOD } else { WARN }, swing.line());
            }
            if woken(&game)
                && let Some(page) = game.chapter.concepts.iter().find(|c| c.found == Found::Side(BEAM_ID.to_string()))
            {
                panel.add_space(6.0);
                if panel.button(format!("Handbook: {}", page.title)).clicked() {
                    ui.open_page(&page.id);
                }
            }
        })
    {
        let r = resp.response.rect;
        ui.occupied.push([r.min.x, r.min.y, r.max.x, r.max.y]);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tipped_beam_rests_an_end_on_its_stop_and_comes_back_level() {
        for swing in [Swing::LoadDown, Swing::FarDown] {
            assert_eq!(tilt_at(swing, 0.0), 0.0);
            assert_eq!(tilt_at(swing, (SETTLE + REST) / 2.0), swing.tilt());
            assert_eq!(tilt_at(swing, BACK), 0.0);
            // It never goes further than its stop.
            for i in 0..200 {
                let t = BACK * i as f32 / 200.0;
                assert!(tilt_at(swing, t).abs() <= 1.0 + 1e-6, "{swing:?} at {t}");
                assert!(tilt_at(swing, t) * swing.tilt() >= 0.0, "{swing:?} turns one way only");
            }
        }
        // Level, it barely moves.
        assert!((0..100).all(|i| tilt_at(Swing::Level, BACK * i as f32 / 100.0).abs() < 0.15));
        assert_eq!(tilt_at(Swing::Level, SETTLE), 0.0);
        // At its stop, the end of the beam's underside is on the ground.
        let end = LENGTH / 2.0;
        let drop = end * most_tilt().sin() + BEAM_THICK / 2.0 * most_tilt().cos();
        assert!((drop - BEAM_AXLE).abs() < 0.02, "{drop}");
    }

    #[test]
    fn the_consoles_view_fits_on_screen() {
        let (_, sites) = crate::island::build(&physics_core::puzzle::Ch1Layout::generate(5));
        let (min, max) = view(&sites.beam);
        let mut orbit = OrbitCamera::default();
        orbit.frame_side_view(min, max, Vec3::NEG_X, false);
        assert!(orbit.distance < crate::player::SIDE_VIEW_FURTHEST, "{}", orbit.distance);
        // The tallest stack the beam can carry is in the box: the machine's load and the player's
        // blocks on it.
        assert!(max.y - sites.beam.pivot.y >= BEAM_THICK / 2.0 + BLOCK * MOST_LOAD as f32 - 0.1);
    }

    #[test]
    fn the_path_lights_only_once_the_beam_has_woken() {
        let (mut world, sites) = crate::island::build(&physics_core::puzzle::Ch1Layout::generate(8));
        let cells: Vec<IVec3> = sites.beam.path.cells.iter().map(|&(c, _)| c).collect();
        assert!(cells.iter().all(|c| !world.is_dark(*c)), "the island is built awake");
        let changes = settle(&mut world, &sites.beam, false);
        assert_eq!(changes.cells.len(), cells.len());
        assert!(cells.iter().all(|c| world.is_dark(*c)));
        // The road's own inlay is the way's, not the beam's: it isn't touched.
        assert!(sites.way.runs.iter().flat_map(|r| &r.cells).all(|(c, _)| !cells.contains(c)));
        settle(&mut world, &sites.beam, true);
        assert!(cells.iter().all(|c| !world.is_dark(*c)));
    }

    #[test]
    fn the_console_is_reached_from_the_path_and_not_from_the_road() {
        let (_, sites) = crate::island::build(&physics_core::puzzle::Ch1Layout::generate(2));
        let beam = &sites.beam;
        assert!(near_console(beam, beam.console.as_vec3() + Vec3::new(0.5, 0.0, 1.8)));
        let road = beam.path.cells.iter().map(|&(c, _)| c).max_by_key(|c| c.z).unwrap();
        assert!(!near_console(beam, road.as_vec3() + Vec3::new(0.5, 1.0, 1.5)));
        // Nor from any station's console.
        for console in sites.stations {
            assert!(!near_console(beam, console.as_vec3()), "the beam's console is apart from the stations'");
        }
    }
}
