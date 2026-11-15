//! Shapes of the things tests move, and of the loose debris by the wreck: built from primitives so a
//! cart reads as a cart and scrap as pieces of a ship. Every shape keeps inside the box the
//! simulation reasons with (a cart's envelope, the beam's size), centred on the entity, so nothing
//! the puzzles measure moves.

use avian3d::prelude::*;
use bevy::prelude::*;
use physics_core::materials::Material;
use std::f32::consts::FRAC_PI_2;

/// What a part is painted with: the colour of what it belongs to, or one of the shared finishes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Paint {
    /// The prop's own colour (the cart's red, the crawler's yellow).
    Body,
    /// Dark iron: wheels, chassis, couplers, bands.
    Iron,
    /// Tread rubber.
    Rubber,
    /// A lamp, glowing.
    Lamp,
}

/// One piece of a shape, placed relative to the entity it belongs to.
#[derive(Clone)]
pub struct Part {
    pub mesh: Handle<Mesh>,
    pub transform: Transform,
    pub paint: Paint,
    /// A wheel of this radius (its axle along z): it turns as the vehicle rolls, and carries spokes
    /// so that can be seen.
    pub wheel: Option<f32>,
}

/// A vehicle: the body is the entity's own mesh, the rest are children.
#[derive(Clone)]
pub struct Vehicle {
    pub body: Handle<Mesh>,
    pub parts: Vec<Part>,
    /// Two crossed bars across a wheel's faces, in the wheel's own frame (axis along y), per wheel
    /// radius.
    spokes: Vec<(f32, Handle<Mesh>)>,
}

/// A wheel that turns with the distance its vehicle travels along x (both tracks run along x).
#[derive(Component)]
pub struct Wheel {
    radius: f32,
    angle: f32,
    last: Option<f32>,
}

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

/// Wheels below a body, `xs` along it, one on each side, standing on the envelope's floor.
fn wheels(meshes: &mut Assets<Mesh>, size: Vec3, radius: f32, width: f32, xs: &[f32], z: f32) -> Vec<Part> {
    let mesh = meshes.add(Cylinder::new(radius, width));
    let y = -size.y / 2.0 + radius;
    xs.iter()
        .flat_map(|&x| [z, -z].map(|z| Part { mesh: mesh.clone(), transform: Transform::from_xyz(x, y, z).with_rotation(axle_along_z()), paint: Paint::Iron, wheel: Some(radius) }))
        .collect()
}

fn spokes(meshes: &mut Assets<Mesh>, radius: f32, width: f32) -> (f32, Handle<Mesh>) {
    // Proud of both faces, and a little short of the rim.
    let bar = Cuboid::new(radius * 1.7, width + 0.02, radius * 0.28);
    let cross = merged([(bar.into(), Transform::default()), (bar.into(), Transform::from_rotation(Quat::from_rotation_y(FRAC_PI_2)))]);
    (radius, meshes.add(cross))
}

/// A cart inside `size`: a tub banded in iron on four spoked wheels, its top flush with the envelope's
/// (the cargo rides there). With couplers, an iron coupler stands at each end of the envelope, and
/// the tub is shortened to make room.
pub fn cart(meshes: &mut Assets<Mesh>, size: Vec3, couplers: bool) -> Vehicle {
    const RADIUS: f32 = 0.17;
    const WIDTH: f32 = 0.09;
    let coupler = if couplers { 0.1 } else { 0.0 };
    let length = size.x - 2.0 * coupler;
    // The tub sits on the axles, with the wheels tucked half under it.
    let height = size.y - RADIUS * 1.1;
    let centre = size.y / 2.0 - height / 2.0;
    let body = meshes.add(Mesh::from(Cuboid::new(length - 0.04, height, size.z - 0.04)).translated_by(Vec3::Y * centre));
    let band = meshes.add(Cuboid::new(length, 0.07, size.z));
    let mut parts = vec![
        Part { mesh: band.clone(), transform: Transform::from_xyz(0.0, size.y / 2.0 - 0.035, 0.0), paint: Paint::Iron, wheel: None },
        Part { mesh: band, transform: Transform::from_xyz(0.0, centre - height / 2.0 + 0.035, 0.0), paint: Paint::Iron, wheel: None },
    ];
    // Corner straps from band to band.
    let strap = meshes.add(Cuboid::new(0.06, height, 0.06));
    for (x, z) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
        let at = Vec3::new(x * (length / 2.0 - 0.03), centre, z * (size.z / 2.0 - 0.03));
        parts.push(Part { mesh: strap.clone(), transform: Transform::from_translation(at), paint: Paint::Iron, wheel: None });
    }
    let xs = [length / 2.0 - 0.2, -(length / 2.0 - 0.2)];
    let z = size.z / 2.0 - WIDTH / 2.0 - 0.02;
    // Axles through the wheels, under the tub.
    let axle = meshes.add(Cylinder::new(0.03, 2.0 * z));
    for x in xs {
        parts.push(Part { mesh: axle.clone(), transform: Transform::from_xyz(x, -size.y / 2.0 + RADIUS, 0.0).with_rotation(axle_along_z()), paint: Paint::Iron, wheel: None });
    }
    parts.extend(wheels(meshes, size, RADIUS, WIDTH, &xs, z));
    if couplers {
        let hitch = meshes.add(Cuboid::new(coupler + 0.04, 0.1, 0.18));
        for x in [1.0, -1.0] {
            let at = Vec3::new(x * (size.x / 2.0 - (coupler + 0.04) / 2.0), centre - height / 2.0 + 0.1, 0.0);
            parts.push(Part { mesh: hitch.clone(), transform: Transform::from_translation(at), paint: Paint::Iron, wheel: None });
        }
    }
    let spokes = vec![spokes(meshes, RADIUS, WIDTH)];
    Vehicle { body, parts, spokes }
}

/// The survey crawler inside `size`: a body on two rubber treads with road wheels, a lamp at the
/// front (`front` is the sign of x it runs towards), a coupler at the rear, an antenna. The top is
/// flush with the envelope's (the guidance unit rides there).
pub fn crawler(meshes: &mut Assets<Mesh>, size: Vec3, front: f32) -> Vehicle {
    const TREAD_H: f32 = 0.32;
    const TREAD_W: f32 = 0.18;
    const COUPLER: f32 = 0.1;
    let tread_y = -size.y / 2.0 + TREAD_H / 2.0;
    let tread_z = size.z / 2.0 - TREAD_W / 2.0;
    // The body overhangs the treads, and stops short of the rear for the coupler and of the front for
    // the lamp.
    const LAMP: f32 = 0.04;
    let height = size.y - TREAD_H + 0.04;
    let length = size.x - COUPLER - LAMP / 2.0;
    let body = meshes.add(Mesh::from(Cuboid::new(length, height, size.z - 0.12)).translated_by(Vec3::new(front * (COUPLER - LAMP / 2.0) / 2.0, size.y / 2.0 - height / 2.0, 0.0)));
    // A tread: a straight run with round ends.
    let end = TREAD_H / 2.0;
    let tread = meshes.add(merged([
        (Cuboid::new(size.x - TREAD_H, TREAD_H, TREAD_W).into(), Transform::default()),
        (Cylinder::new(end, TREAD_W).into(), Transform::from_xyz(size.x / 2.0 - end, 0.0, 0.0).with_rotation(axle_along_z())),
        (Cylinder::new(end, TREAD_W).into(), Transform::from_xyz(-(size.x / 2.0 - end), 0.0, 0.0).with_rotation(axle_along_z())),
    ]));
    let mut parts: Vec<Part> = [tread_z, -tread_z]
        .map(|z| Part { mesh: tread.clone(), transform: Transform::from_xyz(0.0, tread_y, z), paint: Paint::Rubber, wheel: None })
        .into();
    // Road wheels on the treads' outer faces.
    const ROAD: f32 = 0.11;
    let road = meshes.add(Cylinder::new(ROAD, 0.04));
    for x in [-0.42, 0.0, 0.42] {
        for z in [1.0, -1.0] {
            let at = Vec3::new(x * size.x / 1.4, tread_y, z * (size.z / 2.0 - 0.02));
            parts.push(Part { mesh: road.clone(), transform: Transform::from_translation(at).with_rotation(axle_along_z()), paint: Paint::Iron, wheel: Some(ROAD) });
        }
    }
    let rear = -front;
    let hitch = meshes.add(Cuboid::new(COUPLER + 0.04, 0.12, 0.2));
    parts.push(Part { mesh: hitch, transform: Transform::from_xyz(rear * (size.x / 2.0 - (COUPLER + 0.04) / 2.0), tread_y + 0.05, 0.0), paint: Paint::Iron, wheel: None });
    let lamp = meshes.add(Cuboid::new(LAMP, 0.12, 0.3));
    parts.push(Part { mesh: lamp, transform: Transform::from_xyz(front * (size.x / 2.0 - LAMP / 2.0), size.y / 2.0 - height / 2.0 + 0.05, 0.0), paint: Paint::Lamp, wheel: None });
    let mast = meshes.add(Cylinder::new(0.018, 0.55));
    parts.push(Part { mesh: mast, transform: Transform::from_xyz(rear * (size.x / 2.0 - 0.2), size.y / 2.0 + 0.27, size.z / 2.0 - 0.14), paint: Paint::Iron, wheel: None });
    let spokes = vec![spokes(meshes, ROAD, 0.04)];
    Vehicle { body, parts, spokes }
}

impl Vehicle {
    /// Put the vehicle's parts under `parent`, painted by `paint`. With `turning`, its wheels turn as
    /// it moves (an afterimage's do not).
    pub fn spawn_parts(&self, parent: &mut ChildSpawnerCommands, paint: impl Fn(Paint) -> Handle<StandardMaterial>, turning: bool) {
        for part in &self.parts {
            let mut child = parent.spawn((Mesh3d(part.mesh.clone()), MeshMaterial3d(paint(part.paint)), part.transform));
            let Some(radius) = part.wheel else { continue };
            if turning {
                child.insert(Wheel { radius, angle: 0.0, last: None });
            }
            if let Some((_, spokes)) = self.spokes.iter().find(|(r, _)| *r == radius) {
                child.with_child((Mesh3d(spokes.clone()), MeshMaterial3d(paint(Paint::Body))));
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

/// The finishes shared by the vehicles' parts and the debris.
#[derive(Clone)]
pub struct Finishes {
    pub iron: Handle<StandardMaterial>,
    pub rubber: Handle<StandardMaterial>,
    pub lamp: Handle<StandardMaterial>,
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
            rubber: metal(materials, [0.07, 0.07, 0.08], 0.0, 0.9),
            lamp: materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.93, 0.7), emissive: LinearRgba::rgb(3.0, 2.6, 1.6), ..default() }),
            alloy: metal(materials, Material::HullAlloy.properties().colour, 0.6, 0.4),
            copper: metal(materials, Material::Copper.properties().colour, 0.7, 0.35),
            soot: metal(materials, [0.09, 0.08, 0.08], 0.0, 0.95),
            screen: materials.add(StandardMaterial { base_color: Color::srgb(0.12, 0.35, 0.42), emissive: LinearRgba::rgb(0.25, 0.9, 1.2), ..default() }),
        }
    }

    /// The material for a part of a vehicle painted `body`.
    pub fn paint(&self, body: &Handle<StandardMaterial>, paint: Paint) -> Handle<StandardMaterial> {
        match paint {
            Paint::Body => body.clone(),
            Paint::Iron => self.iron.clone(),
            Paint::Rubber => self.rubber.clone(),
            Paint::Lamp => self.lamp.clone(),
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
            let flange = meshes.add(Cylinder::new(radius + 0.08, 0.06));
            let trim = [1.0, -1.0].map(|x| (flange.clone(), finishes.iron.clone(), Transform::from_xyz(x * (length / 2.0 - 0.03), 0.0, 0.0).with_rotation(along)));
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
                    (meshes.add(Cuboid::new(size.x + 0.04, 0.05, size.z + 0.04)), finishes.alloy.clone(), Transform::from_xyz(0.0, size.y / 2.0 - 0.025, 0.0)),
                    (meshes.add(Cylinder::new(0.025, 0.4)), finishes.copper.clone(), Transform::from_xyz(-0.2, -size.y / 2.0 - 0.1, 0.0).with_rotation(Quat::from_rotation_z(0.6))),
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
        assert!((low.y + size.y / 2.0).abs() < 1e-4, "the treads stand on the floor: {low}");
        assert!(low.x >= -size.x / 2.0 - 1e-4 && high.x <= size.x / 2.0 + 1e-4, "the lamp and the coupler keep to the ends: {low} {high}");
        assert!(low.z >= -size.z / 2.0 - 1e-4 && high.z <= size.z / 2.0 + 1e-4, "{low} {high}");
        let (mut top, mut floor) = (Vec3::splat(f32::MIN), Vec3::splat(f32::MAX));
        grow(meshes.get(&crawler.body).unwrap(), Transform::default(), &mut floor, &mut top);
        assert!((top.y - size.y / 2.0).abs() < 1e-4, "the guidance unit rides on the body's top");
    }
}
