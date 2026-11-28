//! Small things in motion: drops thrown up where something falls into the water, dust where a block
//! is set down or stonework moves, spray where the waves meet a cliff, motes of light drifting up
//! from the awake quartz, and the flourishes of a wake (a bright pulse at the head of the light, the
//! motes it leaves behind, a burst as the machine wakes, and the rest of the way's quartz brightening
//! once).
//!
//! A particle is a tiny cube, as everything here is made of blocks, that moves, slows, falls or rises,
//! and shrinks away. They share one mesh and a few materials, so they draw together.
//!
//! What drifts on its own (the motes, the spray) never crosses a measurement: none is made while a
//! test or an experiment runs or the ruler is out, and the few in the air fade within seconds.

use crate::graphics::{glow, glowing};
use crate::island::{SEA_LEVEL, Sites};
use crate::ruler::Ruler;
use crate::scenery::WayGlow;
use crate::sim::Sim;
use crate::state::{BuildWorld, Mode, WorldEntity, not_paused};
use crate::voxel::VoxelWorld;
use crate::waking::{Phase, Waking};
use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use physics_core::materials::Material;

/// What a burst is made of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Drops thrown up where something falls into the water.
    Splash,
    /// A puff of dust where stone meets stone.
    Dust,
    /// A little spray where a wave meets a cliff, or stonework moves in the water.
    Spray,
    /// Motes of the builders' light, as a machine wakes.
    Motes,
}

/// Something happened here that throws up particles. `strength` scales how many, about 1.
#[derive(Message, Clone, Copy, Debug)]
pub struct Burst {
    pub kind: Kind,
    pub at: Vec3,
    pub strength: f32,
}

/// Ask for a burst from anywhere that has `Commands`.
pub fn burst(commands: &mut Commands, kind: Kind, at: Vec3, strength: f32) {
    commands.queue(move |world: &mut World| {
        world.write_message(Burst { kind, at, strength });
    });
}

#[derive(Component)]
struct Particle {
    velocity: Vec3,
    age: f32,
    life: f32,
    size: f32,
    /// Downwards, in metres per second squared; negative rises.
    gravity: f32,
    /// How fast it loses its speed, per second.
    drag: f32,
    /// How far it sways from side to side as it drifts.
    sway: f32,
    /// Whether it is gone once it falls back into the water.
    drowns: bool,
}

/// The bright point at the head of the light while it runs up the road.
#[derive(Component)]
struct Head;

#[derive(Resource)]
struct Looks {
    cube: Handle<Mesh>,
    water: Handle<StandardMaterial>,
    dust: Handle<StandardMaterial>,
    way: Handle<StandardMaterial>,
    wild: Handle<StandardMaterial>,
}

/// A small, fast random number source: particles need variety, not quality.
#[derive(Resource)]
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> f32 {
        // xorshift64*
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        ((self.0.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 40) as f32) / (1u64 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next()
    }

    /// A direction across the ground.
    fn across(&mut self) -> Vec3 {
        let a = self.range(0.0, std::f32::consts::TAU);
        Vec3::new(a.cos(), 0.0, a.sin())
    }
}

/// Where the island's quiet light comes from: the lamps and consoles of the awake machines, and the
/// wild quartz.
#[derive(Resource, Default)]
struct Sources {
    wild: Vec<Vec3>,
}

/// Wake flourish tracking stored as a resource so world reloads reset it cleanly.
#[derive(Resource, Default)]
struct FlourishState {
    last: Option<(usize, Phase)>,
    pulse: Option<f32>,
    was: Option<Vec3>,
}

/// How near the camera may come to the light's bright head before it is hidden, in metres.
const HEAD_CLOSEST: f32 = 6.0;

/// How long the way's quartz takes to brighten and settle as a machine wakes, and by how much.
const PULSE: f32 = 1.6;
const PULSE_LIFT: f32 = 0.5;

/// The most particles in the air at once.
const MOST: usize = 600;
/// The most of those made on their own (motes, spray) in the air at once.
const MOST_AMBIENT: usize = 150;
/// How far from the camera the motes and the spray are made.
const NEAR: f32 = 45.0;

pub struct ParticlesPlugin;

impl Plugin for ParticlesPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<Burst>()
            .insert_resource(Dice(0x9E37_79B9_7F4A_7C15))
            .init_resource::<Sources>()
            .init_resource::<FlourishState>()
            .add_systems(Startup, setup)
            .add_systems(BuildWorld, refresh_sources)
            .add_systems(Update, (wake_flourishes, ambient.run_if(in_state(Mode::Playing)), throw, drift).chain().run_if(not_paused));
    }
}

fn setup(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let quartz = Material::Quartz.properties().colour;
    let stone = Material::Limestone.properties().colour;
    let unlit = |materials: &mut Assets<StandardMaterial>, colour: Color| materials.add(StandardMaterial { base_color: colour, unlit: true, ..default() });
    let looks = Looks {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        water: materials.add(StandardMaterial { base_color: Color::srgb(0.93, 0.98, 0.98), perceptual_roughness: 0.3, ..default() }),
        dust: materials.add(StandardMaterial { base_color: Color::srgb(stone[0], stone[1], stone[2]), perceptual_roughness: 1.0, ..default() }),
        way: unlit(&mut materials, glowing(quartz, glow::WAY * 1.5)),
        wild: unlit(&mut materials, glowing(quartz, glow::WILD * 1.3)),
    };
    commands.spawn((
        Head,
        Mesh3d(meshes.add(Sphere::new(0.24).mesh().ico(2).expect("a small icosphere"))),
        MeshMaterial3d(unlit(&mut materials, glowing(quartz, glow::WAY * 3.0))),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
    ));
    commands.insert_resource(looks);
}

fn refresh_sources(world: Res<VoxelWorld>, mut sources: ResMut<Sources>, mut flourish: ResMut<FlourishState>, mut head: Single<&mut Visibility, With<Head>>) {
    sources.wild = world.wild_cells().map(|c| c.as_vec3() + Vec3::new(0.5, 1.0, 0.5)).collect();
    *flourish = FlourishState::default();
    **head = Visibility::Hidden;
}

/// Throw up what was asked for.
fn throw(mut commands: Commands, mut bursts: MessageReader<Burst>, looks: Res<Looks>, mut dice: ResMut<Dice>, live: Query<(), With<Particle>>) {
    let mut room = MOST.saturating_sub(live.iter().count());
    for burst in bursts.read() {
        let count = match burst.kind {
            Kind::Splash => 16.0,
            Kind::Dust => 10.0,
            Kind::Spray => 5.0,
            Kind::Motes => 40.0,
        } * burst.strength;
        for _ in 0..(count.round() as usize).min(room) {
            let particle = make(burst.kind, &mut dice);
            let material = match burst.kind {
                Kind::Splash | Kind::Spray => looks.water.clone(),
                Kind::Dust => looks.dust.clone(),
                Kind::Motes => looks.way.clone(),
            };
            let at = burst.at + dice.across() * dice.range(0.0, if burst.kind == Kind::Dust { 0.5 } else { 0.3 });
            spawn(&mut commands, &looks, material, at, particle);
            room -= 1;
        }
    }
}

fn spawn(commands: &mut Commands, looks: &Looks, material: Handle<StandardMaterial>, at: Vec3, particle: Particle) {
    let scale = Vec3::splat(particle.size);
    commands.spawn((
        WorldEntity,
        particle,
        Mesh3d(looks.cube.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(at).with_scale(scale),
        NotShadowCaster,
    ));
}

/// How much of its size a particle keeps at this distance from the camera: all of it from two metres
/// out, none of it within half a metre.
fn near_the_eye(distance: f32) -> f32 {
    ((distance - 0.5) / 1.5).clamp(0.0, 1.0)
}

/// One particle of a burst, with its own speed, size and life.
fn make(kind: Kind, dice: &mut Dice) -> Particle {
    let out = dice.across();
    match kind {
        Kind::Splash => Particle {
            velocity: out * dice.range(0.8, 2.4) + Vec3::Y * dice.range(3.0, 5.5),
            age: 0.0,
            life: 1.4,
            size: dice.range(0.10, 0.20),
            gravity: 9.81,
            drag: 0.3,
            sway: 0.0,
            drowns: true,
        },
        Kind::Spray => Particle {
            velocity: out * dice.range(0.3, 0.9) + Vec3::Y * dice.range(1.2, 2.6),
            age: 0.0,
            life: 0.9,
            size: dice.range(0.06, 0.12),
            gravity: 9.81,
            drag: 0.3,
            sway: 0.0,
            drowns: true,
        },
        Kind::Dust => Particle {
            velocity: out * dice.range(0.6, 1.4) + Vec3::Y * dice.range(0.2, 0.6),
            age: 0.0,
            life: dice.range(0.8, 1.3),
            size: dice.range(0.12, 0.24),
            gravity: -0.3,
            drag: 2.5,
            sway: 0.0,
            drowns: false,
        },
        Kind::Motes => Particle {
            velocity: out * dice.range(0.5, 2.0) + Vec3::Y * dice.range(0.8, 2.6),
            age: 0.0,
            life: dice.range(1.4, 2.6),
            size: dice.range(0.05, 0.10),
            gravity: -0.3,
            drag: 1.6,
            sway: 0.15,
            drowns: false,
        },
    }
}

/// A mote that drifts up slowly from a source of light and sways as it goes.
fn mote(dice: &mut Dice, rise: f32, life: f32) -> Particle {
    Particle {
        velocity: dice.across() * dice.range(0.0, 0.08) + Vec3::Y * rise * dice.range(0.7, 1.3),
        age: 0.0,
        life: life * dice.range(0.7, 1.3),
        size: dice.range(0.04, 0.08),
        gravity: 0.0,
        drag: 0.2,
        sway: 0.25,
        drowns: false,
    }
}

fn drift(
    time: Res<Time>,
    mut commands: Commands,
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut particles: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.1);
    let eye = camera.translation();
    for (entity, mut p, mut transform) in &mut particles {
        p.age += dt;
        let falling_in = p.drowns && p.velocity.y < 0.0 && transform.translation.y < SEA_LEVEL;
        if p.age >= p.life || falling_in {
            commands.entity(entity).despawn();
            continue;
        }
        p.velocity.y -= p.gravity * dt;
        let slow = (-p.drag * dt).exp();
        p.velocity *= slow;
        let sway = if p.sway > 0.0 { Vec3::new((p.age * 2.3 + p.size * 97.0).sin(), 0.0, (p.age * 1.7 + p.size * 53.0).cos()) * p.sway } else { Vec3::ZERO };
        transform.translation += (p.velocity + sway) * dt;
        // Full size for most of its life, then shrinking away; and shrinking away as it comes near the
        // camera, where a speck of a few centimetres would fill the picture.
        let u = p.age / p.life;
        transform.scale = Vec3::splat(p.size * (1.0 - u * u * u) * near_the_eye(transform.translation.distance(eye)));
        // The motes turn as they drift, catching the eye now and then.
        if p.sway > 0.0 {
            transform.rotate_y(dt * 1.3);
        }
    }
}

/// The island's quiet motion: motes rising from the awake lamps and consoles and from the wild
/// quartz, and spray where the waves meet a cliff. Only near the camera, and never while something
/// is being measured.
#[allow(clippy::too_many_arguments)]
fn ambient(
    time: Res<Time>,
    mut commands: Commands,
    looks: Res<Looks>,
    sources: Res<Sources>,
    sites: Res<Sites>,
    waking: Res<Waking>,
    world: Res<VoxelWorld>,
    (sim, ruler): (Res<Sim>, Res<Ruler>),
    camera: Single<&GlobalTransform, With<Camera3d>>,
    mut dice: ResMut<Dice>,
    live: Query<(), With<Particle>>,
) {
    if sim.busy() || sim.watching() || ruler.active || live.iter().count() >= MOST_AMBIENT {
        return;
    }
    let dt = time.delta_secs().min(0.1);
    let eye = camera.translation();
    let near = |p: Vec3| p.distance(eye) < NEAR;
    // Each lamp and console a mote every second or two; each wild crystal now and then.
    let lamps = sites.lanterns.iter().zip(&sites.lantern_machine).filter(|&(_, &m)| waking.console_awake(m)).map(|(&at, _)| at + Vec3::Y * 2.5);
    let consoles = sites.stations.iter().enumerate().filter(|&(i, _)| waking.console_awake(i)).map(|(_, c)| c.as_vec3() + Vec3::new(0.5, 1.2, 0.5));
    for at in lamps.chain(consoles).filter(|&p| near(p)) {
        if dice.next() < dt * 0.7 {
            let p = mote(&mut dice, 0.35, 3.5);
            spawn(&mut commands, &looks, looks.way.clone(), at + dice.across() * 0.2, p);
        }
    }
    for &at in sources.wild.iter().filter(|&&p| near(p)) {
        if dice.next() < dt * 0.15 {
            let p = mote(&mut dice, 0.25, 4.0);
            spawn(&mut commands, &looks, looks.wild.clone(), at + dice.across() * 0.4, p);
        }
    }
    // Spray: try a few places near the camera; where one is deep water against a cliff, a wave breaks.
    for _ in 0..3 {
        if dice.next() > dt * 6.0 {
            continue;
        }
        let p = eye + dice.across() * dice.range(4.0, NEAR);
        let (x, z) = (p.x.floor() as i32, p.z.floor() as i32);
        if cliff_foot(&world, x, z) {
            let at = Vec3::new(x as f32 + 0.5, SEA_LEVEL, z as f32 + 0.5);
            for _ in 0..4 {
                let particle = make(Kind::Spray, &mut dice);
                spawn(&mut commands, &looks, looks.water.clone(), at + dice.across() * 0.4, particle);
            }
        }
    }
}

/// Whether a column is deep water right against a cliff: where waves break into spray.
pub fn cliff_foot(world: &VoxelWorld, x: i32, z: i32) -> bool {
    let level = SEA_LEVEL.floor() as i32;
    let inside = |x: i32, z: i32| x >= 0 && z >= 0 && x < world.size.x && z < world.size.z;
    if !inside(x, z) || world.is_solid(IVec3::new(x, level, z)) || crate::water::depth(world, x, z) < 1.5 {
        return false;
    }
    // A wall of rock rising out of the water beside it, not a beach running up from it.
    [IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y]
        .into_iter()
        .any(|d| inside(x + d.x, z + d.y) && world.is_solid(IVec3::new(x + d.x, level + 1, z + d.y)) && world.is_solid(IVec3::new(x + d.x, level + 2, z + d.y)))
}

/// What a wake adds: the bright head of the light with the motes it leaves behind, a burst as the
/// machine wakes, and dust or spray where the way's stonework moves.
#[allow(clippy::too_many_arguments)]
fn wake_flourishes(
    time: Res<Time>,
    mut commands: Commands,
    waking: Res<Waking>,
    sites: Res<Sites>,
    world: Res<VoxelWorld>,
    looks: Res<Looks>,
    mut dice: ResMut<Dice>,
    mut head: Single<(&mut Transform, &mut Visibility), With<Head>>,
    mut flourish: ResMut<FlourishState>,
    (way_glow, mut materials): (Res<WayGlow>, ResMut<Assets<StandardMaterial>>),
    camera: Single<&GlobalTransform, With<Camera3d>>,
) {
    let dt = time.delta_secs().min(0.1);
    let way = &sites.way;
    let (transform, visibility) = &mut *head;
    match waking.head(way) {
        Some(at) if waking.light_running() => {
            let (x, z) = (at.x.floor() as i32, at.y.floor() as i32);
            let ground = world.surface(x, z) as f32;
            let to = Vec3::new(at.x, ground + 0.25, at.y);
            // Moving fast enough that it would skip: lay the motes along the way it came.
            let from = flourish.was.unwrap_or(to);
            flourish.was = Some(to);
            transform.translation = to;
            // Where something stands between them, the camera comes in close to the road; the bright
            // head would fill the picture then, so only its motes show.
            let close = camera.translation().distance(to) < HEAD_CLOSEST;
            visibility.set_if_neq(if close { Visibility::Hidden } else { Visibility::Visible });
            // About sixty a second, spread along the stretch it covered this frame.
            let steps = (from.distance(to) / 0.25).ceil().clamp(1.0, 12.0) as usize;
            for i in 0..steps {
                if dice.next() < dt * 60.0 / steps as f32 {
                    let mut p = mote(&mut dice, 0.2, 1.6);
                    p.size *= 1.4;
                    let along = from.lerp(to, i as f32 / steps as f32);
                    spawn(&mut commands, &looks, looks.way.clone(), along + dice.across() * 0.25, p);
                }
            }
        }
        _ => {
            flourish.was = None;
            visibility.set_if_neq(Visibility::Hidden);
        }
    }

    // The machine wakes: a burst of its light where the run ends.
    let now = waking.play.map(|p| (p.timeline.stage, p.timeline.phase(p.t).0));
    if let Some((stage, Phase::Wake)) = now
        && flourish.last != now
    {
        let run = &way.runs[stage - 1];
        let end = run.at(run.length());
        let ground = world.surface(end.x.floor() as i32, end.y.floor() as i32) as f32;
        burst(&mut commands, Kind::Motes, Vec3::new(end.x, ground + 0.6, end.y), 1.5);
        flourish.pulse = Some(0.0);
    }
    flourish.last = now;

    // As it wakes, all the way's awake quartz brightens once, as if answering it.
    if let Some(t) = flourish.pulse.as_mut() {
        *t += dt;
        let u = (*t / PULSE).min(1.0);
        let lift = 1.0 + PULSE_LIFT * (std::f32::consts::PI * u).sin().powi(2);
        if let Some(mut material) = materials.get_mut(&way_glow.0) {
            material.base_color = Color::LinearRgba(LinearRgba::rgb(lift, lift, lift));
        }
        if u >= 1.0 {
            flourish.pulse = None;
        }
    }

    // Stonework on the move raises dust, or spray where it is in the water.
    if let Some(play) = waking.play {
        for work in way.stonework() {
            let moved = waking.moved(work);
            if work.stage != play.timeline.stage || moved <= 0.0 || moved >= 1.0 || dice.next() > dt * 12.0 {
                continue;
            }
            let (cell, _) = work.cells[(dice.next() * work.cells.len() as f32) as usize % work.cells.len()];
            let wet = (cell.y as f32) < SEA_LEVEL;
            let at = if wet { Vec3::new(cell.x as f32 + 0.5, SEA_LEVEL, cell.z as f32 + 0.5) } else { cell.as_vec3() + Vec3::new(0.5, 0.0, 0.5) };
            burst(&mut commands, if wet { Kind::Spray } else { Kind::Dust }, at, 0.6);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particles_shrink_away_at_the_camera() {
        assert_eq!(near_the_eye(0.3), 0.0);
        assert_eq!(near_the_eye(2.0), 1.0);
        assert!(near_the_eye(1.0) > 0.0 && near_the_eye(1.0) < 1.0);
    }

    #[test]
    fn the_dice_stay_in_range_and_vary() {
        let mut dice = Dice(1);
        let rolls: Vec<f32> = (0..1000).map(|_| dice.next()).collect();
        assert!(rolls.iter().all(|r| (0.0..1.0).contains(r)));
        let mean = rolls.iter().sum::<f32>() / rolls.len() as f32;
        assert!((mean - 0.5).abs() < 0.05, "{mean}");
    }

    #[test]
    fn every_particle_shrinks_away_within_its_life() {
        let mut dice = Dice(7);
        for kind in [Kind::Splash, Kind::Dust, Kind::Spray, Kind::Motes] {
            let p = make(kind, &mut dice);
            assert!(p.life > 0.0 && p.life < 3.0, "{kind:?} lives {}", p.life);
            assert!(p.size > 0.0 && p.size < 0.3, "{kind:?} is {} across", p.size);
        }
    }

    #[test]
    fn spray_breaks_at_cliffs_and_not_on_beaches() {
        let level = SEA_LEVEL.floor() as i32;
        let mut world = VoxelWorld::new(IVec3::new(12, level + 6, 12));
        // A floor four blocks under the surface; a cliff on the west, a beach on the east.
        world.fill(IVec3::new(0, 0, 0), IVec3::new(11, level - 5, 11), Material::Sand);
        world.fill(IVec3::new(0, 0, 0), IVec3::new(1, level + 4, 11), Material::Rock);
        for (i, x) in (8..12).enumerate() {
            world.fill(IVec3::new(x, 0, 0), IVec3::new(x, level - 3 + 2 * i as i32, 11), Material::Sand);
        }
        assert!(cliff_foot(&world, 2, 5), "beside the cliff");
        assert!(!cliff_foot(&world, 5, 5), "open water");
        assert!(!cliff_foot(&world, 9, 5), "the beach");
    }
}
