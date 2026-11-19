//! Shapes of the things tests move, and of the loose debris by the wreck: built from primitives so a
//! cart reads as a cart and scrap as pieces of a ship. Every shape keeps inside the box the
//! simulation reasons with (a cart's envelope, the tiller's size), centred on the entity, so nothing
//! the puzzles measure moves.

use avian3d::prelude::*;
use bevy::prelude::*;
use physics_core::materials::Material;
use std::f32::consts::{FRAC_PI_2, PI, TAU};

/// What a part is painted with: the colour of what it belongs to, or one of the shared finishes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paint {
    /// The same stone, worn darker where it works: wheels, legs, couplers, lips.
    Worn,
    /// Quartz set into one of the builders' machines: it sleeps and wakes with the machine (the
    /// caller picks the material; see `waking::MachineTrim`).
    Trim,
}

/// How a part moves with its vehicle.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Motion {
    Fixed,
    /// A wheel of this radius (its axle along z): it turns as the vehicle rolls, and carries a cross
    /// of quartz so that can be seen.
    Wheel(f32),
    /// A leg hung from its hip (the part's own origin): it swings fore and aft and lifts on the
    /// forward swing as the vehicle walks. Legs with the same phase step together.
    Leg(f32),
}

/// One piece of a shape, placed relative to the entity it belongs to.
#[derive(Clone)]
pub struct Part {
    pub mesh: Handle<Mesh>,
    pub transform: Transform,
    pub paint: Paint,
    pub motion: Motion,
}

impl Part {
    fn fixed(mesh: &Handle<Mesh>, transform: Transform, paint: Paint) -> Self {
        Part { mesh: mesh.clone(), transform, paint, motion: Motion::Fixed }
    }
}

/// A vehicle: the body is the entity's own mesh, the rest are children.
#[derive(Clone)]
pub struct Vehicle {
    pub body: Handle<Mesh>,
    pub parts: Vec<Part>,
    /// Two crossed bars of quartz across a wheel's faces, in the wheel's own frame (axis along y), per
    /// wheel radius.
    spokes: Vec<(f32, Handle<Mesh>)>,
}

/// A wheel that turns with the distance its vehicle travels along x (both tracks run along x).
#[derive(Component)]
pub struct Wheel {
    radius: f32,
    angle: f32,
    last: Option<f32>,
}

/// A leg that steps with the distance its vehicle travels along x.
#[derive(Component)]
pub struct Leg {
    rest: Transform,
    phase: f32,
    walked: f32,
    last: Option<f32>,
}

/// How far the crawler goes in one full step of a leg, how far its legs reach below the hips, and how
/// high a foot lifts as it swings forward.
const STRIDE: f32 = 0.44;
const LEG_H: f32 = 0.34;
const LIFT: f32 = 0.05;

/// Lying on its side: a cylinder's axis turned from y to z.
fn axle_along_z() -> Quat {
    Quat::from_rotation_x(FRAC_PI_2)
}

/// One mesh of several, each placed by its transform.
pub fn merged(pieces: impl IntoIterator<Item = (Mesh, Transform)>) -> Mesh {
    let mut pieces = pieces.into_iter().map(|(mesh, transform)| mesh.transformed_by(transform));
    let mut whole = pieces.next().expect("at least one piece");
    for piece in pieces {
        whole.merge(&piece).expect("primitive meshes share their attributes");
    }
    whole
}

/// The builders' casket the helm travels in, inside `size`: a stone box with a heavier lid, and a
/// band of quartz round its middle. Returns the stone and the quartz.
pub fn casket(size: Vec3) -> [Mesh; 2] {
    let lid = size.y * 0.2;
    let body = merged([
        (Cuboid::new(size.x * 0.94, size.y - lid, size.z * 0.94).into(), Transform::from_xyz(0.0, -lid / 2.0, 0.0)),
        (Cuboid::new(size.x, lid, size.z).into(), Transform::from_xyz(0.0, size.y / 2.0 - lid / 2.0, 0.0)),
    ]);
    // Proud of the box's sides, short of the lid's overhang.
    let band = Cuboid::new(size.x * 0.97, size.y * 0.12, size.z * 0.97).into();
    [body, merged([(band, Transform::from_xyz(0.0, -lid / 2.0, 0.0))])]
}

/// The skiff's helm, along x inside `size`: a square stone shaft with a grip at its -x end and the
/// rudder's blade at its +x end, and quartz running along the shaft's top and set through the
/// blade. Returns the stone and the quartz.
pub fn tiller(size: Vec3) -> [Mesh; 2] {
    let shaft = size.y * 0.5;
    let blade_x = size.x * 0.32;
    let blade_thick = size.z * 0.34;
    let shaft_len = size.x - blade_x * 0.6;
    let shaft_mid = -size.x / 2.0 + shaft_len / 2.0;
    let stone = merged([
        (Cuboid::new(shaft_len, shaft, shaft).into(), Transform::from_xyz(shaft_mid, 0.0, 0.0)),
        // The grip: a little thicker than the shaft.
        (Cuboid::new(size.x * 0.12, shaft * 1.25, shaft * 1.25).into(), Transform::from_xyz(-size.x / 2.0 + size.x * 0.06, 0.0, 0.0)),
        // The blade: as tall as the envelope, thin across it.
        (Cuboid::new(blade_x, size.y, blade_thick).into(), Transform::from_xyz(size.x / 2.0 - blade_x / 2.0, 0.0, 0.0)),
    ]);
    let quartz = merged([
        (Cuboid::new(shaft_len * 0.7, 0.04, shaft * 0.4).into(), Transform::from_xyz(shaft_mid, shaft / 2.0 + 0.02, 0.0)),
        (Cuboid::new(blade_x * 0.5, size.y * 0.55, blade_thick + 0.03).into(), Transform::from_xyz(size.x / 2.0 - blade_x / 2.0, 0.0, 0.0)),
    ]);
    [stone, quartz]
}

/// An I-beam along x, as long, tall and wide as `size`: two flanges and a web.
pub fn i_beam(size: Vec3) -> Mesh {
    let flange = (size.y * 0.16).max(0.05);
    let web = (size.z * 0.18).max(0.05);
    let y = size.y / 2.0 - flange / 2.0;
    merged([
        (Cuboid::new(size.x, flange, size.z).into(), Transform::from_xyz(0.0, y, 0.0)),
        (Cuboid::new(size.x, flange, size.z).into(), Transform::from_xyz(0.0, -y, 0.0)),
        (Cuboid::new(size.x, size.y - 2.0 * flange, web).into(), Transform::default()),
    ])
}

/// Solid stone wheels below a body, `xs` along it, one on each side, standing on the envelope's floor.
fn wheels(meshes: &mut Assets<Mesh>, size: Vec3, radius: f32, width: f32, xs: &[f32], z: f32) -> Vec<Part> {
    let mesh = meshes.add(Cylinder::new(radius, width));
    let y = -size.y / 2.0 + radius;
    xs.iter()
        .flat_map(|&x| [z, -z].map(|z| Part { mesh: mesh.clone(), transform: Transform::from_xyz(x, y, z).with_rotation(axle_along_z()), paint: Paint::Worn, motion: Motion::Wheel(radius) }))
        .collect()
}

/// The quartz cross set into a wheel's faces.
fn spokes(meshes: &mut Assets<Mesh>, radius: f32, width: f32) -> (f32, Handle<Mesh>) {
    // Proud of both faces, and a little short of the rim.
    let bar = Cuboid::new(radius * 1.7, width + 0.02, radius * 0.22);
    let cross = merged([(bar.into(), Transform::default()), (bar.into(), Transform::from_rotation(Quat::from_rotation_y(FRAC_PI_2)))]);
    (radius, meshes.add(cross))
}

/// The builders' cart inside `size`: a smooth stone tub with a worn stone lip round its top, a
/// channel of quartz along each side, on four solid stone wheels with quartz crosses. The lip is
/// flush with the envelope's top (the cargo rides there). With couplers, a stone coupler stands at
/// each end of the envelope, and the tub is shortened to make room.
///
/// No two faces of different finishes share a plane, or the renderer flickers between them: the tub
/// sits inside its lip and a little below its top, and the wheels stand between the tub's sides and
/// the lip's faces.
pub fn cart(meshes: &mut Assets<Mesh>, size: Vec3, couplers: bool) -> Vehicle {
    const RADIUS: f32 = 0.17;
    const WIDTH: f32 = 0.09;
    const LIP: f32 = 0.07;
    const RIM: f32 = 0.06;
    let coupler = if couplers { 0.1 } else { 0.0 };
    let length = size.x - 2.0 * coupler;
    // The tub sits on the axles, with the wheels tucked half under it.
    let height = size.y - RADIUS * 1.1;
    let floor = size.y / 2.0 - height;
    let top = size.y / 2.0;
    let (tub_low, tub_high) = (floor + 0.01, top - 0.02);
    let tub_half_z = (size.z - 0.08) / 2.0;
    let body = meshes.add(Mesh::from(Cuboid::new(length - 0.06, tub_high - tub_low, 2.0 * tub_half_z)).translated_by(Vec3::Y * (tub_low + tub_high) / 2.0));
    // The lip: a rim of worn stone round the tub's top.
    let rim_long = meshes.add(Cuboid::new(length, LIP, RIM));
    let rim_short = meshes.add(Cuboid::new(RIM, LIP, size.z - 2.0 * RIM));
    let rim_y = top - LIP / 2.0;
    let mut parts = vec![
        Part::fixed(&rim_long, Transform::from_xyz(0.0, rim_y, size.z / 2.0 - RIM / 2.0), Paint::Worn),
        Part::fixed(&rim_long, Transform::from_xyz(0.0, rim_y, -(size.z / 2.0 - RIM / 2.0)), Paint::Worn),
        Part::fixed(&rim_short, Transform::from_xyz(length / 2.0 - RIM / 2.0, rim_y, 0.0), Paint::Worn),
        Part::fixed(&rim_short, Transform::from_xyz(-(length / 2.0 - RIM / 2.0), rim_y, 0.0), Paint::Worn),
    ];
    let xs = [length / 2.0 - 0.2, -(length / 2.0 - 0.2)];
    // The wheels' outer faces between the tub's sides and the lip's, their crosses short of the lip.
    let z = size.z / 2.0 - WIDTH / 2.0 - 0.02;
    // Stone axles through the wheels, under the tub.
    let axle = meshes.add(Cylinder::new(0.03, 2.0 * z));
    for x in xs {
        parts.push(Part::fixed(&axle, Transform::from_xyz(x, -size.y / 2.0 + RADIUS, 0.0).with_rotation(axle_along_z()), Paint::Worn));
    }
    parts.extend(wheels(meshes, size, RADIUS, WIDTH, &xs, z));
    // A channel of quartz along each side of the tub, set just proud of its face.
    let strip = meshes.add(Cuboid::new(length * 0.6, 0.06, 0.012));
    let strip_y = (floor + top) / 2.0 + 0.04;
    for side in [1.0, -1.0] {
        parts.push(Part::fixed(&strip, Transform::from_xyz(0.0, strip_y, side * (tub_half_z + 0.008)), Paint::Trim));
    }
    if couplers {
        let hitch = meshes.add(Cuboid::new(coupler + 0.04, 0.1, 0.18));
        for x in [1.0, -1.0] {
            parts.push(Part::fixed(&hitch, Transform::from_xyz(x * (size.x / 2.0 - (coupler + 0.04) / 2.0), floor + 0.1, 0.0), Paint::Worn));
        }
    }
    let spokes = vec![spokes(meshes, RADIUS, WIDTH)];
    Vehicle { body, parts, spokes }
}

/// The builders' crawler inside `size`: a stone body that walks on six stone legs, three a side,
/// stepping in two alternating threes, quartz at each knee and in a channel along each flank, a
/// quartz eye at the front (`front` is the sign of x it walks towards) and a stone coupler at the
/// rear. The top is flush with the envelope's (the sight rides there); the feet, at rest, stand on
/// its floor.
pub fn crawler(meshes: &mut Assets<Mesh>, size: Vec3, front: f32) -> Vehicle {
    const LEG_W: f32 = 0.09;
    const COUPLER: f32 = 0.1;
    const EYE: f32 = 0.04;
    let hip_y = -size.y / 2.0 + LEG_H;
    // The legs stand outside the body's flanks, inside the envelope.
    let leg_z = size.z / 2.0 - LEG_W / 2.0 - 0.02;
    let body_half_z = leg_z - LEG_W / 2.0 - 0.04;
    // The body stops short of the rear for the coupler and of the front for the eye; its foot sits a
    // little below the hips, between the legs.
    let height = size.y / 2.0 - hip_y + 0.03;
    let length = size.x - COUPLER - EYE / 2.0;
    let body = meshes.add(Mesh::from(Cuboid::new(length, height, 2.0 * body_half_z)).translated_by(Vec3::new(front * (COUPLER - EYE / 2.0) / 2.0, size.y / 2.0 - height / 2.0, 0.0)));
    // A leg: a stone shank hung from the hip, with a block of quartz at the knee, both swinging from
    // the hip.
    let shank = meshes.add(Mesh::from(Cuboid::new(LEG_W, LEG_H, LEG_W)).translated_by(Vec3::Y * -LEG_H / 2.0));
    let knee = meshes.add(Mesh::from(Cuboid::new(LEG_W + 0.03, 0.07, LEG_W + 0.03)).translated_by(Vec3::Y * -LEG_H * 0.45));
    let mut parts = Vec::new();
    for (i, x) in [-0.36, 0.0, 0.36].into_iter().enumerate() {
        for (j, side) in [1.0, -1.0].into_iter().enumerate() {
            // Two alternating threes: front and rear on one side with the middle on the other.
            let phase = if (i + j) % 2 == 0 { 0.0 } else { PI };
            let hip = Transform::from_xyz(x * size.x / 1.4, hip_y, side * leg_z);
            parts.push(Part { mesh: shank.clone(), transform: hip, paint: Paint::Worn, motion: Motion::Leg(phase) });
            parts.push(Part { mesh: knee.clone(), transform: hip, paint: Paint::Trim, motion: Motion::Leg(phase) });
        }
    }
    // A channel of quartz along each flank, just proud of it.
    let strip = meshes.add(Cuboid::new(length * 0.7, 0.05, 0.012));
    for side in [1.0, -1.0] {
        parts.push(Part::fixed(&strip, Transform::from_xyz(front * (COUPLER - EYE / 2.0) / 2.0, size.y / 2.0 - height * 0.45, side * (body_half_z + 0.008)), Paint::Trim));
    }
    let rear = -front;
    let hitch = meshes.add(Cuboid::new(COUPLER + 0.04, 0.12, 0.2));
    parts.push(Part::fixed(&hitch, Transform::from_xyz(rear * (size.x / 2.0 - (COUPLER + 0.04) / 2.0), hip_y + 0.05, 0.0), Paint::Worn));
    // Its eye at the front: quartz. The crawler never slept; it still walks its rounds.
    let eye = meshes.add(Cuboid::new(EYE, 0.12, 0.3));
    parts.push(Part::fixed(&eye, Transform::from_xyz(front * (size.x / 2.0 - EYE / 2.0), size.y / 2.0 - height / 2.0 + 0.05, 0.0), Paint::Trim));
    Vehicle { body, parts, spokes: Vec::new() }
}

impl Vehicle {
    /// Put the vehicle's parts under `parent`, painted by `paint`. With `moving`, its wheels turn and
    /// its legs step as it moves (an afterimage's do not).
    pub fn spawn_parts(&self, parent: &mut ChildSpawnerCommands, paint: impl Fn(Paint) -> Handle<StandardMaterial>, moving: bool) {
        for part in &self.parts {
            let mut child = parent.spawn((Mesh3d(part.mesh.clone()), MeshMaterial3d(paint(part.paint)), part.transform));
            match part.motion {
                Motion::Fixed => {}
                Motion::Leg(phase) => {
                    if moving {
                        child.insert(Leg { rest: part.transform, phase, walked: 0.0, last: None });
                    }
                }
                Motion::Wheel(radius) => {
                    if moving {
                        child.insert(Wheel { radius, angle: 0.0, last: None });
                    }
                    if let Some((_, spokes)) = self.spokes.iter().find(|(r, _)| *r == radius) {
                        child.with_child((Mesh3d(spokes.clone()), MeshMaterial3d(paint(Paint::Trim))));
                    }
                }
            }
        }
    }
}

/// Turn each wheel by the distance it travelled along x since the last frame. A jump of more than a
/// metre is a vehicle put back at its start, not rolled there.
pub fn spin_wheels(mut wheels: Query<(&mut Wheel, &mut Transform, &GlobalTransform)>) {
    for (mut wheel, mut transform, global) in &mut wheels {
        let x = global.translation().x;
        if let Some(last) = wheel.last {
            let dx = x - last;
            if dx.abs() < 1.0 {
                // Rolling towards +x turns a wheel clockwise seen from +z.
                wheel.angle -= dx / wheel.radius;
            }
        }
        wheel.last = Some(x);
        transform.rotation = Quat::from_rotation_z(wheel.angle) * axle_along_z();
    }
}

/// Where a leg is in its step after its vehicle has walked `walked` metres along x: its swing about
/// z (positive puts the foot towards +x) and how high its foot is lifted. For the first half of each
/// step the foot is planted and sweeps back exactly as fast as the body goes on, so it does not
/// slide; for the second half it lifts and swings forward again.
pub fn step(walked: f32, phase: f32) -> (f32, f32) {
    let u = (walked / STRIDE + phase / TAU).rem_euclid(1.0);
    let reach = STRIDE / 4.0;
    let (ahead, lift) = if u < 0.5 {
        (reach - u * STRIDE, 0.0)
    } else {
        let a = TAU * (u - 0.5);
        (-reach * a.cos(), LIFT * a.sin())
    };
    ((ahead / LEG_H).clamp(-1.0, 1.0).asin(), lift)
}

/// Step each leg by the distance its vehicle walked along x since the last frame (a jump of more
/// than a metre is a vehicle put back at its start). Planted feet sweep back as the body goes on, and
/// lifted ones swing forward.
pub fn step_legs(mut legs: Query<(&mut Leg, &mut Transform, &GlobalTransform)>) {
    for (mut leg, mut transform, global) in &mut legs {
        let x = global.translation().x;
        if let Some(last) = leg.last {
            let dx = x - last;
            if dx.abs() < 1.0 {
                leg.walked += dx;
            }
        }
        leg.last = Some(x);
        let (swing, lift) = step(leg.walked, leg.phase);
        transform.rotation = leg.rest.rotation * Quat::from_rotation_z(swing);
        transform.translation = leg.rest.translation + Vec3::Y * lift;
    }
}

/// The finishes shared by the vehicles' parts and the debris.
#[derive(Clone)]
pub struct Finishes {
    pub iron: Handle<StandardMaterial>,
    /// The builders' stone worn darker where it works: wheels, legs, couplers, lips.
    pub worn: Handle<StandardMaterial>,
    pub alloy: Handle<StandardMaterial>,
    pub copper: Handle<StandardMaterial>,
    pub soot: Handle<StandardMaterial>,
    pub screen: Handle<StandardMaterial>,
}

impl Finishes {
    pub fn new(materials: &mut Assets<StandardMaterial>) -> Self {
        let metal = |materials: &mut Assets<StandardMaterial>, c: [f32; 3], metallic: f32, roughness: f32| {
            materials.add(StandardMaterial { base_color: Color::srgb(c[0], c[1], c[2]), metallic, perceptual_roughness: roughness, ..default() })
        };
        Self {
            iron: metal(materials, [0.17, 0.18, 0.20], 0.5, 0.55),
            worn: metal(materials, [0.60, 0.56, 0.48], 0.0, 0.9),
            alloy: metal(materials, Material::HullAlloy.properties().colour, 0.6, 0.4),
            copper: metal(materials, Material::Copper.properties().colour, 0.7, 0.35),
            soot: metal(materials, [0.09, 0.08, 0.08], 0.0, 0.95),
            screen: materials.add(StandardMaterial { base_color: Color::srgb(0.12, 0.35, 0.42), emissive: LinearRgba::rgb(0.25, 0.9, 1.2), ..default() }),
        }
    }

    /// The material for a part of a vehicle whose own stone is `body`.
    pub fn paint(&self, body: &Handle<StandardMaterial>, paint: Paint) -> Handle<StandardMaterial> {
        match paint {
            Paint::Worn => self.worn.clone(),
            Paint::Trim => body.clone(),
        }
    }
}

/// A piece of loose wreckage: its collider, its main mesh and material, and pieces in other finishes.
pub struct Debris {
    pub collider: Collider,
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    pub trim: Vec<(Handle<Mesh>, Handle<StandardMaterial>, Transform)>,
}

impl Debris {
    /// How far the piece reaches below its centre as built: what it rests on when it lies unturned.
    pub fn depth(&self, meshes: &Assets<Mesh>) -> f32 {
        let mut lowest = lowest_y(meshes.get(&self.mesh).expect("the debris mesh is added"), Transform::default());
        for (mesh, _, transform) in &self.trim {
            lowest = lowest.min(lowest_y(meshes.get(mesh).expect("trim meshes are added"), *transform));
        }
        -lowest
    }
}

/// The lowest point of `mesh` placed by `transform`.
fn lowest_y(mesh: &Mesh, transform: Transform) -> f32 {
    use bevy::mesh::VertexAttributeValues;
    let Some(VertexAttributeValues::Float32x3(points)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { return 0.0 };
    points.iter().map(|p| transform.transform_point(Vec3::from(*p)).y).fold(f32::MAX, f32::min)
}

/// The kinds of wreckage thrown clear of the hull, `kind` taken modulo their number.
pub fn debris(meshes: &mut Assets<Mesh>, finishes: &Finishes, kind: usize) -> Debris {
    match kind % 5 {
        // A hull panel bent along a seam, stiffened by ribs underneath, its torn edge scorched.
        0 => {
            let bend = Quat::from_rotation_z(0.45);
            let (half, thick, depth) = (0.55, 0.06, 0.85);
            let flat = Transform::from_xyz(-half / 2.0, 0.0, 0.0);
            let tilted = Transform::from_translation(bend * Vec3::new(half / 2.0, 0.0, 0.0)).with_rotation(bend);
            let rib = |t: Transform, z: f32| (Mesh::from(Cuboid::new(half, 0.1, 0.05)), t * Transform::from_xyz(0.0, -0.07, z));
            let mesh = merged([
                (Cuboid::new(half, thick, depth).into(), flat),
                (Cuboid::new(half, thick, depth).into(), tilted),
                rib(flat, 0.25),
                rib(flat, -0.25),
                rib(tilted, 0.25),
                rib(tilted, -0.25),
            ]);
            let edge = tilted * Transform::from_xyz(half / 2.0 - 0.03, 0.0, 0.0);
            Debris {
                collider: Collider::compound(vec![
                    (flat.translation, flat.rotation, Collider::cuboid(half, thick + 0.1, depth)),
                    (tilted.translation, tilted.rotation, Collider::cuboid(half, thick + 0.1, depth)),
                ]),
                mesh: meshes.add(mesh),
                material: finishes.alloy.clone(),
                trim: vec![(meshes.add(Cuboid::new(0.08, thick + 0.02, depth + 0.02)), finishes.soot.clone(), edge)],
            }
        }
        // A length of copper conduit with iron flanges at its ends.
        1 => {
            let (radius, length) = (0.13, 1.1);
            let along = Quat::from_rotation_z(FRAC_PI_2);
            // Proud of the pipe's ends, so their faces do not share its caps' planes.
            let flange = meshes.add(Cylinder::new(radius + 0.08, 0.06));
            let trim = [1.0, -1.0].map(|x| (flange.clone(), finishes.iron.clone(), Transform::from_xyz(x * (length / 2.0 - 0.02), 0.0, 0.0).with_rotation(along)));
            Debris {
                collider: Collider::compound(vec![(Vec3::ZERO, along, Collider::cylinder(radius + 0.08, length))]),
                mesh: meshes.add(Mesh::from(Cylinder::new(radius, length)).rotated_by(along)),
                material: finishes.copper.clone(),
                trim: trim.into(),
            }
        }
        // A stub of girder.
        2 => {
            let size = Vec3::new(1.2, 0.34, 0.28);
            Debris { collider: Collider::cuboid(size.x, size.y, size.z), mesh: meshes.add(i_beam(size)), material: finishes.alloy.clone(), trim: Vec::new() }
        }
        // A thruster bell, sooty inside, with its mounting collar.
        3 => {
            let (top, bottom, height) = (0.2, 0.42, 0.6);
            let bell = ConicalFrustum { radius_top: top, radius_bottom: bottom, height };
            Debris {
                collider: Collider::cylinder(bottom, height + 0.12),
                mesh: meshes.add(bell),
                material: finishes.alloy.clone(),
                trim: vec![
                    (meshes.add(Cylinder::new(top + 0.05, 0.12)), finishes.iron.clone(), Transform::from_xyz(0.0, height / 2.0 + 0.05, 0.0)),
                    (meshes.add(Cylinder::new(bottom - 0.03, 0.01)), finishes.soot.clone(), Transform::from_xyz(0.0, -height / 2.0 - 0.006, 0.0)),
                ],
            }
        }
        // A console torn out of the bridge, its screen still faintly lit.
        _ => {
            let size = Vec3::new(0.75, 0.48, 0.32);
            Debris {
                collider: Collider::cuboid(size.x, size.y, size.z),
                mesh: meshes.add(Cuboid::from_size(size)),
                material: finishes.iron.clone(),
                trim: vec![
                    (meshes.add(Cuboid::new(0.56, 0.3, 0.02)), finishes.screen.clone(), Transform::from_xyz(0.0, 0.03, size.z / 2.0 + 0.005)),
                    (meshes.add(Cuboid::new(size.x + 0.04, 0.05, size.z + 0.04)), finishes.alloy.clone(), Transform::from_xyz(0.0, size.y / 2.0 - 0.015, 0.0)),
                    // A torn cable out of its back.
                    (meshes.add(Cylinder::new(0.025, 0.4)), finishes.copper.clone(), Transform::from_xyz(-0.2, -0.1, -size.z / 2.0 - 0.17).with_rotation(Quat::from_rotation_x(FRAC_PI_2 - 0.4))),
                ],
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::VertexAttributeValues;

    /// The corners of the box around every vertex of `mesh` placed by `t`, grown into `low` and `high`.
    fn grow(mesh: &Mesh, t: Transform, low: &mut Vec3, high: &mut Vec3) {
        let Some(VertexAttributeValues::Float32x3(points)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!("no positions") };
        for p in points {
            let p = t.transform_point(Vec3::from(*p));
            *low = low.min(p);
            *high = high.max(p);
        }
    }

    fn bounds(meshes: &Assets<Mesh>, vehicle: &Vehicle) -> (Vec3, Vec3) {
        let (mut low, mut high) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        grow(meshes.get(&vehicle.body).unwrap(), Transform::default(), &mut low, &mut high);
        for part in &vehicle.parts {
            grow(meshes.get(&part.mesh).unwrap(), part.transform, &mut low, &mut high);
        }
        (low, high)
    }

    /// The triangles of `mesh` placed by `t` whose normal lies along an axis, as (axis, facing +,
    /// plane offset, low and high corners in the plane).
    fn flat_faces(mesh: &Mesh, t: Transform) -> Vec<(usize, bool, f32, Vec2, Vec2)> {
        let Some(VertexAttributeValues::Float32x3(points)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { panic!("no positions") };
        let points: Vec<Vec3> = points.iter().map(|p| t.transform_point(Vec3::from(*p))).collect();
        let indices: Vec<usize> = mesh.indices().expect("indexed").iter().collect();
        let mut faces = Vec::new();
        for tri in indices.chunks(3) {
            let [a, b, c] = [points[tri[0]], points[tri[1]], points[tri[2]]];
            let normal = (b - a).cross(c - a);
            if normal.length() < 1e-8 {
                continue;
            }
            let normal = normal.normalize();
            let Some(axis) = (0..3).find(|&i| normal[i].abs() > 1.0 - 1e-4) else { continue };
            let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
            let flat = |p: Vec3| Vec2::new(p[u], p[v]);
            let (low, high) = (flat(a).min(flat(b)).min(flat(c)), flat(a).max(flat(b)).max(flat(c)));
            faces.push((axis, normal[axis] > 0.0, a[axis], low, high));
        }
        faces
    }

    /// No two faces of different finishes lie in one plane, facing the same way and overlapping: the
    /// renderer would flicker between them.
    #[test]
    fn vehicles_have_no_flickering_faces() {
        let mut meshes = Assets::<Mesh>::default();
        let vehicles = [cart(&mut meshes, Vec3::new(0.9, 0.7, 0.9), false), cart(&mut meshes, Vec3::new(1.2, 0.7, 0.9), true), crawler(&mut meshes, Vec3::new(1.4, 0.9, 0.9), 1.0)];
        for vehicle in &vehicles {
            // The body is painted with the vehicle's own stone (`None`).
            let mut pieces = vec![(None, meshes.get(&vehicle.body).unwrap(), Transform::default())];
            for part in &vehicle.parts {
                pieces.push((Some(part.paint), meshes.get(&part.mesh).unwrap(), part.transform));
                let Motion::Wheel(radius) = part.motion else { continue };
                if let Some((_, spokes)) = vehicle.spokes.iter().find(|(s, _)| *s == radius) {
                    pieces.push((Some(Paint::Trim), meshes.get(spokes).unwrap(), part.transform));
                }
            }
            let faces: Vec<_> = pieces.iter().map(|(paint, mesh, t)| (*paint, flat_faces(mesh, *t))).collect();
            for (i, (paint_a, a)) in faces.iter().enumerate() {
                for (paint_b, b) in &faces[i + 1..] {
                    if paint_a == paint_b {
                        continue;
                    }
                    for fa in a {
                        for fb in b {
                            let same_plane = fa.0 == fb.0 && fa.1 == fb.1 && (fa.2 - fb.2).abs() < 1e-3;
                            let overlap = fa.3.max(fb.3);
                            let overlap = fa.4.min(fb.4) - overlap;
                            assert!(!(same_plane && overlap.x > 1e-3 && overlap.y > 1e-3), "{paint_a:?} and {paint_b:?} share the plane {:?} = {}", fa.0, fa.2);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn vehicles_keep_to_their_envelopes() {
        let mut meshes = Assets::<Mesh>::default();
        let size = Vec3::new(0.9, 0.7, 0.9);
        for couplers in [false, true] {
            let vehicle = cart(&mut meshes, size, couplers);
            let (low, high) = bounds(&meshes, &vehicle);
            // The wheels stand on the floor, the deck is the top, and nothing sticks out.
            assert!((low.y + size.y / 2.0).abs() < 1e-4, "{low}");
            assert!((high.y - size.y / 2.0).abs() < 1e-4, "{high}");
            assert!(low.cmpge(-size / 2.0 - 1e-4).all() && high.cmple(size / 2.0 + 1e-4).all(), "{low} {high}");
            if couplers {
                assert!((high.x - size.x / 2.0).abs() < 1e-4 && (low.x + size.x / 2.0).abs() < 1e-4, "the couplers are at the ends");
            }
        }
        let size = Vec3::new(1.4, 0.9, 0.9);
        let crawler = crawler(&mut meshes, size, -1.0);
        let (low, high) = bounds(&meshes, &crawler);
        assert!((low.y + size.y / 2.0).abs() < 1e-4, "the feet stand on the floor: {low}");
        assert!(low.x >= -size.x / 2.0 - 1e-4 && high.x <= size.x / 2.0 + 1e-4, "the eye and the coupler keep to the ends: {low} {high}");
        assert!(low.z >= -size.z / 2.0 - 1e-4 && high.z <= size.z / 2.0 + 1e-4, "{low} {high}");
        let (mut top, mut floor) = (Vec3::splat(f32::MIN), Vec3::splat(f32::MAX));
        grow(meshes.get(&crawler.body).unwrap(), Transform::default(), &mut floor, &mut top);
        assert!((top.y - size.y / 2.0).abs() < 1e-4, "the sight rides on the body's top");
        // It walks: six legs, nothing that rolls, and no part of the castaway's (iron, rubber).
        assert_eq!(crawler.parts.iter().filter(|p| matches!(p.motion, Motion::Leg(_)) && p.paint == Paint::Worn).count(), 6);
        assert!(crawler.parts.iter().all(|p| !matches!(p.motion, Motion::Wheel(_))));
    }

    /// A planted foot keeps still on the ground while the body walks on, either way: it sweeps back
    /// exactly as fast as the body goes, and it is down for half of each step.
    #[test]
    fn a_planted_foot_does_not_slide() {
        let foot_x = |walked: f32| walked + LEG_H * step(walked, 0.0).0.sin();
        let n = 400;
        let mut planted = 0;
        for i in 0..n {
            let walked = 3.0 * STRIDE * i as f32 / n as f32;
            if step(walked, 0.0).1 == 0.0 && step(walked + 0.002, 0.0).1 == 0.0 {
                planted += 1;
                assert!((foot_x(walked + 0.002) - foot_x(walked)).abs() < 1e-4, "the foot slides at {walked}");
            }
            let back = -walked;
            if step(back, 0.0).1 == 0.0 && step(back - 0.002, 0.0).1 == 0.0 {
                assert!((foot_x(back - 0.002) - foot_x(back)).abs() < 1e-4, "walking back, the foot slides at {back}");
            }
        }
        assert!((planted as f32 / n as f32 - 0.5).abs() < 0.03, "down for {planted} of {n}");
        // The two threes are never both up.
        for i in 0..n {
            let walked = STRIDE * i as f32 / n as f32;
            assert!(step(walked, 0.0).1 == 0.0 || step(walked, PI).1 == 0.0, "all six feet up at {walked}");
        }
    }
}
