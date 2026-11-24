//! The title menu and, behind it, the intro in attract mode: the crash that left the castaway on the
//! island, played on a loop. The timeline is in [`crate::cinematic`]; this module builds the sets,
//! moves the ship and the camera along it, and draws the menu. Start hands the camera to the player.
//!
//! The sets for levels III and II are placeholders: a planet seen from orbit, and the mainland across
//! the strait with the main wreck on it. The mainland stays in the world after Start, as the far
//! shore of the strait; everything else the intro made goes when the game begins.

use crate::cinematic::{self, Event, Frame, GLIDE, LOOP, PLANET_RADIUS, Part, Sets, Trail};
use crate::island::{SEA_LEVEL, Sites};
use crate::player::{OrbitCamera, Player};
use crate::scenery::SkyDome;
use crate::state::Mode;
use crate::voxel::{VoxelWorld, fbm, hash01, smoothstep};
use avian3d::prelude::{Physics, PhysicsTime};
use bevy::asset::RenderAssetUsages;
use bevy::light::NotShadowCaster;
use bevy::mesh::{Indices, PrimitiveTopology, VertexAttributeValues};
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use bevy_egui::{EguiContexts, EguiPrimaryContextPass, egui};
use egui::{Align2, Color32, RichText, vec2};
use physics_core::materials::Material;

/// How long the picture takes to fade out after Start when the intro is not over the island.
const LEAVE: f32 = 0.5;
/// How often a burning or smoking part leaves a puff behind.
const PUFF_EVERY: f32 = 0.03;

/// Level II's placeholder: the mainland across the strait, with the main wreck on it. Its blocks are
/// two metres, so it reads as a larger land than the island.
pub struct Mainland {
    world: VoxelWorld,
}

const MAINLAND_SIZE: IVec3 = IVec3::new(128, 40, 128);
const MAINLAND_SCALE: f32 = 2.0;
/// Corner of the mainland's grid in the world: north-west of the island, over the strait. Its own
/// sea level (8.6 blocks up) sits on the island's, as the distant islands' do.
const MAINLAND_ORIGIN: Vec3 = Vec3::new(-240.0, SEA_LEVEL - SEA_LEVEL * MAINLAND_SCALE, -448.0);
/// Column where the main hull lies, and the height of the plain around it, in the grid.
const MAINLAND_SITE: IVec2 = IVec2::new(64, 60);
const MAINLAND_PLAIN: i32 = 12;

impl Mainland {
    /// The mainland, with the gouge the main hull cut running back from the wreck, away from `island`.
    pub fn new(island: Vec3) -> Self {
        let mut world = VoxelWorld::new(MAINLAND_SIZE);
        let site = MAINLAND_SITE.as_vec2() + Vec2::splat(0.5);
        let heading = (island - Self::to_world(site.extend(0.0).xzy())).xz().normalize();
        // The hull came in along this line: a plain, so nothing stands in its way.
        let (from, to) = (site - heading * 80.0, site + heading * 6.0);
        for z in 0..MAINLAND_SIZE.z {
            for x in 0..MAINLAND_SIZE.x {
                let (xf, zf) = (x as f32, z as f32);
                let at = Vec2::new(xf + 0.5, zf + 0.5);
                let d = (at - Vec2::new(64.0, 64.0)).length() / 54.0 + (fbm(xf / 16.0, zf / 16.0, 1201, 3) - 0.5) * 0.55;
                let rise = (1.0 - d).max(0.0);
                let hills = fbm(xf / 11.0, zf / 11.0, 1211, 3);
                let line = distance_to_segment(at, from, to);
                let open = smoothstep(8.0, 22.0, line);
                let mut top =
                    if d >= 1.0 { (7.5 - (d - 1.0) * 12.0).max(2.0) } else { 9.0 + 3.0 * smoothstep(0.0, 0.2, rise) + rise.powf(0.9) * hills * 26.0 * open };
                if d < 1.0 {
                    top += (MAINLAND_PLAIN as f32 - top) * (1.0 - smoothstep(6.0, 12.0, line));
                }
                let top = top.round() as i32;
                let surface = if top <= 9 {
                    Material::Sand
                } else if top > 26 && hash01(IVec3::new(x, 1203, z)) < 0.7 {
                    Material::Rock
                } else {
                    Material::Grass
                };
                for y in 0..top {
                    let material = match top - 1 - y {
                        0 => surface,
                        1..=2 if surface == Material::Grass => Material::Soil,
                        1..=2 => surface,
                        _ => Material::Rock,
                    };
                    world.set(IVec3::new(x, y, z), material);
                }
                // A few trees, away from the hull's line.
                if surface == Material::Grass && line > 14.0 && hash01(IVec3::new(x, 1205, z)) < 0.025 {
                    world.fill(IVec3::new(x, top, z), IVec3::new(x, top + 2, z), Material::Timber);
                    world.fill(IVec3::new(x - 1, top + 3, z - 1), IVec3::new(x + 1, top + 3, z + 1), Material::Foliage);
                }
            }
        }
        // The gouge the hull cut behind it, the scorched ground around, then the hull itself: a long
        // broken box of plating, lying along the line it came in on, where the model comes down.
        let across = heading.perp();
        let column = |along: f32, side: f32| {
            let at = site + heading * along + across * side;
            (at.x.floor() as i32, at.y.floor() as i32)
        };
        for step in -70..=-8 {
            for side in -4_i32..=4 {
                let (x, z) = column(step as f32 * 0.5, side as f32 * 0.5);
                let top = IVec3::new(x, world.surface(x, z) - 1, z);
                if top.y == MAINLAND_PLAIN - 1 {
                    world.set(top, Material::Air);
                    world.set(top - IVec3::Y, Material::Soil);
                }
            }
        }
        for step in -80..=12 {
            for side in -12_i32..=12 {
                let (x, z) = column(step as f32 * 0.5, side as f32 * 0.5);
                let top = IVec3::new(x, world.surface(x, z) - 1, z);
                if world.get(top) == Material::Grass && hash01(top) * 6.0 > (side.abs() as f32 * 0.5) - 1.5 {
                    world.set(top, Material::Soil);
                }
            }
        }
        for along in -3..=4 {
            for side in -1..=1 {
                let (x, z) = column(along as f32, side as f32);
                let cell = IVec3::new(x, world.surface(x, z), z);
                let height = if along == 4 || hash01(cell) < 0.2 { 1 } else { 2 };
                world.fill(cell, cell + IVec3::Y * (height - 1), Material::HullAlloy);
            }
        }
        Mainland { world }
    }

    fn to_world(local: Vec3) -> Vec3 {
        MAINLAND_ORIGIN + local * MAINLAND_SCALE
    }

    /// Where the main hull comes down: the foot of the wreck, on the plain. It needs only the
    /// mainland's constants, so places that look across the strait can know it without building it.
    pub fn wreck() -> Vec3 {
        Self::to_world(Vec3::new(MAINLAND_SITE.x as f32 + 0.5, MAINLAND_PLAIN as f32, MAINLAND_SITE.y as f32 + 0.5))
    }

    /// Height of the mainland's surface under a point in the world (sea level off its shores).
    #[cfg_attr(not(test), expect(dead_code, reason = "the tests check the intro's paths against it"))]
    pub fn ground(&self, p: Vec3) -> f32 {
        let local = (p - MAINLAND_ORIGIN) / MAINLAND_SCALE;
        let top = self.world.surface(local.x.floor() as i32, local.z.floor() as i32);
        (MAINLAND_ORIGIN.y + top as f32 * MAINLAND_SCALE).max(SEA_LEVEL)
    }
}

fn distance_to_segment(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// One of the three pieces of the ship.
#[derive(Component)]
struct ShipPart(Part);

/// The stars, shown only in orbit. They follow the camera, as the sky does.
#[derive(Component)]
struct StarField;

/// A puff of fire, smoke or dust: it drifts, grows or shrinks, and goes.
#[derive(Component)]
struct Puff {
    age: f32,
    life: f32,
    size: f32,
    velocity: Vec3,
    grows: bool,
}

#[derive(Resource)]
struct PuffAssets {
    mesh: Handle<Mesh>,
    fire: Handle<StandardMaterial>,
    smoke: Handle<StandardMaterial>,
    dust: Handle<StandardMaterial>,
}

enum Phase {
    /// The intro plays on a loop behind the menu.
    Attract,
    /// Start was pressed away from the island: the picture fades out.
    Leaving { t: f32 },
    /// The camera glides to the player's view.
    Glide { t: f32, from: (Vec3, Vec3), fade_in: bool },
}

/// The intro's state.
#[derive(Resource)]
pub struct Intro {
    sets: Sets,
    clock: f32,
    phase: Phase,
    frame: Frame,
    start: bool,
    /// The clear colour and haze of the sky, kept while the orbit shot replaces them.
    sky: Option<(Color, DistanceFog)>,
    in_space: Option<bool>,
    puff_clock: f32,
    dice: i32,
}

impl Intro {
    /// Start the game, as the button does.
    pub fn request_start(&mut self) {
        self.start = true;
    }

    /// Seconds into the loop, while the intro plays.
    pub fn clock(&self) -> Option<f32> {
        matches!(self.phase, Phase::Attract).then_some(self.clock)
    }

    fn roll(&mut self) -> f32 {
        self.dice += 1;
        hash01(IVec3::new(self.dice, 811, 37))
    }

    fn jitter(&mut self) -> Vec3 {
        Vec3::new(self.roll() - 0.5, self.roll() - 0.5, self.roll() - 0.5) * 2.0
    }
}

pub struct IntroPlugin;

impl Plugin for IntroPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<Mode>()
            .add_systems(Startup, setup)
            .add_systems(Update, (play, pose_ship, backdrop, drift_puffs).chain().run_if(in_state(Mode::Menu)))
            .add_systems(EguiPrimaryContextPass, menu.after(crate::ui::style).run_if(in_state(Mode::Menu)))
            .add_systems(OnEnter(Mode::Playing), hand_over);
    }
}

#[allow(clippy::too_many_arguments)]
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut physics: ResMut<Time<Physics>>,
    world: Res<VoxelWorld>,
    sites: Res<Sites>,
    orbit: Res<OrbitCamera>,
) {
    let mainland = Mainland::new(sites.wreck);
    let focus = sites.spawn + Vec3::Y * 1.5;
    let sets = Sets::new(&world, sites.wreck, sites.spawn, Mainland::wreck(), (orbit.eye(focus), focus));

    // The mainland, lit and coloured as the island is.
    let blocks = materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.95, ..default() });
    commands.spawn((
        Mesh3d(meshes.add(mainland.world.mesh(0, MAINLAND_SIZE.x, 0, MAINLAND_SIZE.z, false))),
        MeshMaterial3d(blocks.clone()),
        Transform::from_translation(MAINLAND_ORIGIN).with_scale(Vec3::splat(MAINLAND_SCALE)),
        NotShadowCaster,
    ));

    // Level III's placeholder: the planet from orbit, and stars.
    let intro_only = || DespawnOnExit(Mode::Menu);
    commands.spawn((
        intro_only(),
        Mesh3d(meshes.add(planet())),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.85, ..default() })),
        Transform::from_translation(sets.planet()).with_rotation(Quat::from_rotation_z(0.3)),
        NotShadowCaster,
    ));
    commands.spawn((
        intro_only(),
        StarField,
        Mesh3d(meshes.add(stars())),
        MeshMaterial3d(materials.add(StandardMaterial { base_color: Color::WHITE, unlit: true, fog_enabled: false, cull_mode: None, ..default() })),
        Transform::default(),
        Visibility::Hidden,
        NotShadowCaster,
    ));

    // The ship, in the three pieces it breaks into.
    let c = Material::HullAlloy.properties().colour;
    let alloy = materials.add(StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), metallic: 0.6, perceptual_roughness: 0.4, ..default() });
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.16, 0.18, 0.22), perceptual_roughness: 0.7, ..default() });
    let window = materials.add(StandardMaterial { base_color: Color::srgb(0.55, 0.8, 0.95), perceptual_roughness: 0.2, ..default() });
    let glow = materials.add(StandardMaterial { base_color: Color::srgb(0.65, 0.88, 1.0), unlit: true, ..default() });
    for part in Part::ALL {
        let mut boxes: Vec<(Vec3, Vec3, &Handle<StandardMaterial>)> = Vec::new();
        match part {
            Part::Section => {
                boxes.push((Vec3::new(5.6, 3.8, 7.0), Vec3::new(0.0, 0.0, 1.0), &alloy));
                boxes.push((Vec3::new(4.4, 3.0, 2.0), Vec3::new(0.0, -0.2, -3.5), &alloy));
                boxes.push((Vec3::new(3.8, 0.7, 0.3), Vec3::new(0.0, 0.7, -4.4), &window));
                boxes.push((Vec3::new(5.8, 0.5, 7.2), Vec3::new(0.0, -1.7, 1.0), &dark));
            }
            Part::Hull => {
                boxes.push((Vec3::new(5.6, 4.2, 14.0), Vec3::ZERO, &alloy));
                boxes.push((Vec3::new(1.2, 0.8, 10.0), Vec3::new(0.0, 2.4, 0.5), &dark));
                for side in [-1.0, 1.0] {
                    boxes.push((Vec3::new(0.4, 2.2, 4.0), Vec3::new(side * 3.0, -0.6, 4.5), &dark));
                }
            }
            Part::Drive => {
                boxes.push((Vec3::new(5.0, 4.0, 0.6), Vec3::new(0.0, 0.0, -3.7), &alloy));
                boxes.push((Vec3::new(4.6, 3.6, 6.0), Vec3::new(0.0, 0.0, -0.4), &dark));
                for x in [-1.5, 0.0, 1.5] {
                    boxes.push((Vec3::new(1.3, 1.3, 1.6), Vec3::new(x, -0.4, 3.2), &alloy));
                    boxes.push((Vec3::new(0.9, 0.9, 0.1), Vec3::new(x, -0.4, 4.05), &glow));
                }
            }
        }
        commands.spawn((intro_only(), ShipPart(part), Transform::default(), Visibility::Hidden)).with_children(|body| {
            for (size, centre, material) in boxes {
                body.spawn((Mesh3d(meshes.add(Cuboid::from_size(size))), MeshMaterial3d(material.clone()), Transform::from_translation(centre)));
            }
        });
    }

    let translucent = |colour: Color, unlit: bool| StandardMaterial { base_color: colour, alpha_mode: AlphaMode::Blend, unlit, ..default() };
    commands.insert_resource(PuffAssets {
        mesh: meshes.add(Cuboid::default()),
        fire: materials.add(translucent(Color::srgba(1.0, 0.62, 0.22, 0.85), true)),
        smoke: materials.add(translucent(Color::srgba(0.30, 0.29, 0.32, 0.55), false)),
        dust: materials.add(translucent(Color::srgba(0.62, 0.56, 0.48, 0.6), false)),
    });

    // The loose wreckage near the island's wreck waits for the section to come down.
    physics.pause();
    commands.insert_resource(Intro {
        frame: cinematic::frame_at(&sets, 0.0),
        sets,
        clock: 0.0,
        phase: Phase::Attract,
        start: false,
        sky: None,
        in_space: None,
        puff_clock: 0.0,
        dice: 0,
    });
}

/// A planet seen from orbit: sea, teal land with violet patches, ice at the poles and cloud.
fn planet() -> Mesh {
    let mut mesh = Sphere::new(PLANET_RADIUS).mesh().uv(96, 48);
    let colours: Vec<[f32; 4]> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(positions)) => positions
            .iter()
            .map(|p| {
                let n = Vec3::from_array(*p) / PLANET_RADIUS;
                let land = 0.5 * (fbm(n.x * 2.5 + 7.0, n.z * 2.5, 1301, 4) + fbm(n.y * 2.5, n.x * 2.5 + 3.0, 1302, 4));
                let cloud = 0.5 * (fbm(n.x * 4.0, n.y * 4.0 + 11.0, 1303, 3) + fbm(n.z * 4.0, n.y * 4.0, 1304, 3));
                let sea = Vec3::new(0.07, 0.30, 0.42).lerp(Vec3::new(0.10, 0.42, 0.50), smoothstep(0.4, 0.52, land));
                let ground = Vec3::new(0.30, 0.60, 0.52).lerp(Vec3::new(0.50, 0.42, 0.66), smoothstep(0.62, 0.72, land));
                let mut c = sea.lerp(ground, smoothstep(0.53, 0.55, land));
                c = c.lerp(Vec3::new(0.90, 0.93, 0.96), smoothstep(0.82, 0.88, n.y.abs()));
                c = c.lerp(Vec3::ONE, smoothstep(0.56, 0.68, cloud) * 0.85);
                let linear = Color::srgb(c.x, c.y, c.z).to_linear();
                [linear.red, linear.green, linear.blue, 1.0]
            })
            .collect(),
        _ => Vec::new(),
    };
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colours);
    mesh
}

/// Small squares scattered over a sphere around the camera, facing it.
fn stars() -> Mesh {
    let (mut positions, mut colours, mut indices) = (Vec::new(), Vec::new(), Vec::new());
    for i in 0..700 {
        let r = |k: i32| hash01(IVec3::new(i, 1401 + k, 5));
        let (y, angle) = (r(0) * 2.0 - 1.0, r(1) * std::f32::consts::TAU);
        let direction = Vec3::new((1.0 - y * y).sqrt() * angle.cos(), y, (1.0 - y * y).sqrt() * angle.sin());
        let size = 0.6 + 1.4 * r(2) * r(2);
        let (u, v) = direction.any_orthonormal_pair();
        let centre = direction * 870.0;
        let first = positions.len() as u32;
        for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            positions.push((centre + (u * a + v * b) * size).to_array());
        }
        let light = 0.5 + 0.5 * r(3);
        colours.extend([[light, light, light * (0.9 + 0.2 * r(4)), 1.0]; 4]);
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colours)
        .with_inserted_indices(Indices::U32(indices))
}

fn spawn_puff(commands: &mut Commands, assets: &PuffAssets, kind: Trail, at: Vec3, velocity: Vec3, size: f32, life: f32) {
    let material = match kind {
        Trail::Fire => assets.fire.clone(),
        Trail::Smoke => assets.smoke.clone(),
        Trail::Dust | Trail::None => assets.dust.clone(),
    };
    commands.spawn((
        DespawnOnExit(Mode::Menu),
        Puff { age: 0.0, life, size, velocity, grows: kind != Trail::Fire },
        Mesh3d(assets.mesh.clone()),
        MeshMaterial3d(material),
        Transform::from_translation(at).with_scale(Vec3::splat(size * 0.4)),
        NotShadowCaster,
    ));
}

/// Dust thrown up and out where something hits the ground.
fn burst(commands: &mut Commands, assets: &PuffAssets, intro: &mut Intro, at: Vec3, count: usize, speed: f32, size: f32) {
    for i in 0..count {
        let angle = (i as f32 + intro.roll() * 0.6) / count as f32 * std::f32::consts::TAU;
        let out = Vec3::new(angle.cos(), 0.2 + 0.6 * intro.roll(), angle.sin()) * speed * (0.4 + intro.roll());
        let (grain, life) = (size * (0.6 + 0.8 * intro.roll()), 2.5 + 2.0 * intro.roll());
        spawn_puff(commands, assets, Trail::Dust, at + Vec3::Y * size * 0.3, out, grain, life);
    }
    for _ in 0..count / 3 {
        let out = intro.jitter() * speed * 0.5 + Vec3::Y * speed * 0.4;
        spawn_puff(commands, assets, Trail::Fire, at + Vec3::Y, out, size * 0.8, 0.8);
    }
}

#[allow(clippy::too_many_arguments)]
fn play(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    mut commands: Commands,
    mut intro: ResMut<Intro>,
    assets: Res<PuffAssets>,
    mut physics: ResMut<Time<Physics>>,
    mut next: ResMut<NextState<Mode>>,
    mut camera: Single<&mut Transform, With<Camera3d>>,
) {
    let dt = time.delta_secs().min(0.25);
    let intro = &mut *intro;
    if !wants.wants_any_keyboard_input() && keys.any_just_pressed([KeyCode::Enter, KeyCode::NumpadEnter, KeyCode::Space]) {
        intro.start = true;
    }

    if let Phase::Glide { t, from, .. } = &mut intro.phase {
        *t += dt;
        let (eye, look) = cinematic::glide(*from, intro.sets.play, *t);
        camera.translation = eye;
        camera.look_at(look, Vec3::Y);
        if *t >= GLIDE {
            next.set(Mode::Playing);
        }
        return;
    }

    let before = intro.clock;
    intro.clock = (intro.clock + dt).rem_euclid(LOOP);
    intro.frame = cinematic::frame_at(&intro.sets, intro.clock);
    for event in cinematic::events_between(before, intro.clock) {
        match event {
            Event::Breakup => {
                let hull = intro.frame.parts[Part::Hull as usize];
                let joint = hull.at + hull.rotation * Vec3::Z * 7.0;
                for _ in 0..14 {
                    let out = intro.jitter() * 6.0;
                    spawn_puff(&mut commands, &assets, Trail::Fire, joint, out, 1.6, 0.6);
                }
            }
            Event::HullImpact => {
                let at = intro.sets.hull_rest - Vec3::Y * Part::Hull.size().y / 2.0;
                burst(&mut commands, &assets, intro, at, 40, 22.0, 8.0);
            }
            Event::Touchdown => {
                let at = intro.sets.wreck;
                burst(&mut commands, &assets, intro, at, 48, 12.0, 3.5);
                physics.unpause();
            }
            Event::CastawayAppears => {}
        }
    }

    // Burning and smoking parts leave puffs behind them.
    intro.puff_clock += dt;
    while intro.puff_clock >= PUFF_EVERY {
        intro.puff_clock -= PUFF_EVERY;
        for part in Part::ALL {
            let pose = intro.frame.parts[part as usize];
            if !pose.shown || pose.trail == Trail::None {
                continue;
            }
            let half = part.size().z / 2.0;
            let nose = pose.at + pose.rotation * Vec3::NEG_Z * half;
            let tail = pose.at + pose.rotation * Vec3::Z * half;
            let drift = intro.jitter() * 1.5;
            match pose.trail {
                Trail::Fire => {
                    let size = 2.5 + 2.0 * intro.roll();
                    spawn_puff(&mut commands, &assets, Trail::Fire, nose + intro.jitter(), drift, size, 0.7);
                    spawn_puff(&mut commands, &assets, Trail::Smoke, tail + intro.jitter(), drift, size, 3.5);
                }
                Trail::Smoke | Trail::Dust => spawn_puff(&mut commands, &assets, pose.trail, tail + intro.jitter(), drift, 2.5, 3.0),
                Trail::None => {}
            }
        }
    }

    let frame = &intro.frame;
    let wobble = Vec3::new((intro.clock * 37.0).sin(), (intro.clock * 41.0 + 1.0).sin(), (intro.clock * 43.0 + 2.0).sin());
    camera.translation = frame.eye + wobble * frame.shake * 0.6;
    camera.look_at(frame.look, Vec3::Y);

    if let Phase::Leaving { t } = &mut intro.phase {
        *t += dt;
        if *t >= LEAVE {
            // Covered: cut to the island and glide in.
            intro.phase = Phase::Glide { t: 0.0, from: intro.sets.hold, fade_in: true };
            physics.unpause();
        }
    } else if std::mem::take(&mut intro.start) {
        intro.phase =
            if cinematic::settled(intro.clock) { Phase::Glide { t: 0.0, from: (frame.eye, frame.look), fade_in: false } } else { Phase::Leaving { t: 0.0 } };
    }
}

fn pose_ship(intro: Res<Intro>, mut parts: Query<(&ShipPart, &mut Transform, &mut Visibility)>) {
    let gliding = matches!(intro.phase, Phase::Glide { .. });
    for (part, mut transform, mut visibility) in &mut parts {
        let pose = intro.frame.parts[part.0 as usize];
        let shown = pose.shown && !gliding;
        visibility.set_if_neq(if shown { Visibility::Inherited } else { Visibility::Hidden });
        if shown {
            transform.translation = pose.at;
            transform.rotation = pose.rotation;
        }
    }
}

/// The sky or space behind the picture, and whether the castaway is there yet.
#[allow(clippy::type_complexity)]
fn backdrop(
    mut intro: ResMut<Intro>,
    mut clear: ResMut<ClearColor>,
    camera: Single<(&Transform, &mut DistanceFog), With<Camera3d>>,
    mut dome: Single<&mut Visibility, (With<SkyDome>, Without<StarField>, Without<Player>)>,
    mut stars: Single<(&mut Transform, &mut Visibility), (With<StarField>, Without<Camera3d>, Without<Player>)>,
    mut avatar: Single<&mut Visibility, (With<Player>, Without<SkyDome>, Without<StarField>)>,
) {
    let (eye, mut fog) = camera.into_inner();
    let gliding = matches!(intro.phase, Phase::Glide { .. });
    let space = intro.frame.space && !gliding;
    let (sky_colour, sky_fog) = intro.sky.get_or_insert_with(|| (clear.0, fog.clone())).clone();
    if intro.in_space != Some(space) {
        intro.in_space = Some(space);
        if space {
            clear.0 = Color::srgb(0.01, 0.012, 0.025);
            fog.color = sky_fog.color.with_alpha(0.0);
            fog.directional_light_color = Color::NONE;
        } else {
            clear.0 = sky_colour;
            *fog = sky_fog;
        }
        **dome = if space { Visibility::Hidden } else { Visibility::Inherited };
        *stars.1 = if space { Visibility::Inherited } else { Visibility::Hidden };
    }
    stars.0.translation = eye.translation;
    let castaway = intro.frame.castaway || !matches!(intro.phase, Phase::Attract);
    avatar.set_if_neq(if castaway { Visibility::Inherited } else { Visibility::Hidden });
}

fn drift_puffs(time: Res<Time>, intro: Res<Intro>, mut commands: Commands, mut puffs: Query<(Entity, &mut Puff, &mut Transform)>) {
    let dt = time.delta_secs().min(0.25);
    let gliding = matches!(intro.phase, Phase::Glide { .. });
    for (entity, mut puff, mut transform) in &mut puffs {
        puff.age += dt;
        if puff.age >= puff.life || gliding {
            commands.entity(entity).despawn();
            continue;
        }
        let drag = (-1.8 * dt).exp();
        puff.velocity *= drag;
        if puff.grows {
            puff.velocity.y += 0.8 * dt;
        }
        transform.translation += puff.velocity * dt;
        let u = puff.age / puff.life;
        let scale = if puff.grows { 0.4 + 1.4 * u.sqrt() } else { 1.0 - 0.8 * u };
        transform.scale = Vec3::splat(puff.size * scale * (1.0 - smoothstep(0.8, 1.0, u) * 0.6));
    }
}

/// Once the game has begun: the world moves again and the castaway is there.
fn hand_over(mut physics: ResMut<Time<Physics>>, mut avatar: Single<&mut Visibility, With<Player>>) {
    physics.unpause();
    **avatar = Visibility::Inherited;
}

fn menu(mut contexts: EguiContexts, mut intro: ResMut<Intro>) -> Result {
    let ctx = contexts.ctx_mut()?.clone();
    let screen = ctx.content_rect();
    let frame = &intro.frame;
    // How much of the menu shows: all of it while the intro plays, less as it hands over.
    let (shown, fade, fade_colour) = match intro.phase {
        Phase::Attract => (1.0, frame.fade, frame.fade_colour),
        Phase::Leaving { t } => (1.0 - t / LEAVE, frame.fade.max(t / LEAVE), [0.02, 0.025, 0.04]),
        Phase::Glide { t, fade_in, .. } => (0.0, if fade_in { 1.0 - smoothstep(0.0, 0.7, t) } else { 0.0 }, [0.02, 0.025, 0.04]),
    };
    let bars = match intro.phase {
        Phase::Glide { t, .. } => 1.0 - smoothstep(0.0, GLIDE * 0.8, t),
        _ => 1.0,
    };

    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("intro")));
    cinematic::paint_veil(&painter, screen, fade, fade_colour);
    if shown > 0.0 {
        cinematic::paint_veil(&painter, screen, frame.flash.min(1.0) * shown, [1.0, 0.97, 0.9]);
    }

    // Letterbox bars, with the caption in the lower one.
    let heading = frame.caption.map(|caption| format!("{}  ·  {}", caption.stop.numeral(), caption.stop.title().to_uppercase()));
    let alpha = frame.caption.map_or(0.0, |caption| caption.alpha * shown);
    cinematic::paint_letterbox(&painter, screen, bars, heading.as_deref(), frame.caption.map(|c| c.line), alpha);

    if shown <= 0.0 {
        return Ok(());
    }
    let mut start = false;
    egui::Area::new(egui::Id::new("title")).anchor(Align2::LEFT_CENTER, [56.0, -10.0]).show(&ctx, |ui| {
        ui.set_opacity(shown);
        ui.label(RichText::new("First Principles").size(46.0).strong().color(Color32::from_rgb(240, 242, 245)));
        ui.add_space(18.0);
        let button = egui::Button::new(RichText::new("Start").size(22.0).strong()).min_size(vec2(170.0, 42.0));
        start = ui.add(button).clicked();
        ui.add_space(4.0);
        ui.label(RichText::new("or press Enter").small().color(Color32::from_rgb(200, 205, 212)));
    });
    if start && matches!(intro.phase, Phase::Attract) {
        intro.request_start();
    }
    Ok(())
}
