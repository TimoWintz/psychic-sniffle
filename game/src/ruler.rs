//! The ruler: click two points, read their horizontal and vertical separation.
//!
//! It measures once: after the second point it stops taking clicks, and the reading stays on screen
//! until it is put away or a new measurement starts.
//!
//! It snaps to survey pins (the points puzzles are built around) when the cursor is near one, and
//! otherwise to the centre of the block face under the cursor.

use crate::island::Sites;
use crate::voxel::VoxelWorld;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;

#[derive(Clone, Debug, PartialEq)]
pub struct Picked {
    pub point: Vec3,
    pub label: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measurement {
    pub horizontal: f32,
    pub vertical: f32,
    pub direct: f32,
}

#[derive(Resource, Default)]
pub struct Ruler {
    /// Taking points: clicks in the world go to the ruler.
    pub active: bool,
    pub first: Option<Picked>,
    pub second: Option<Picked>,
    pub hover: Option<Picked>,
}

impl Ruler {
    pub fn measurement(&self) -> Option<Measurement> {
        let (a, b) = (self.first.as_ref()?.point, self.second.as_ref()?.point);
        let d = b - a;
        Some(Measurement { horizontal: d.with_y(0.0).length(), vertical: d.y.abs(), direct: d.length() })
    }

    /// Start a new measurement, replacing any reading on screen.
    pub fn start(&mut self) {
        *self = Ruler { active: true, ..Ruler::default() };
    }

    /// Stop measuring and clear the reading.
    pub fn put_away(&mut self) {
        *self = Ruler::default();
    }

    /// Whether the ruler is on screen: taking points, or showing a reading.
    pub fn in_use(&self) -> bool {
        self.active || self.first.is_some()
    }

    /// Take a point: the first, then the second, which ends the measurement. The reading stays.
    pub fn pick(&mut self, picked: Picked) {
        if self.first.is_none() || self.second.is_some() {
            self.first = Some(picked);
            self.second = None;
        } else {
            self.second = Some(picked);
            self.active = false;
            self.hover = None;
        }
    }
}

/// How close, in screen pixels, the cursor must be to a pin to snap to it.
const PIN_SNAP_PIXELS: f32 = 22.0;

pub struct RulerPlugin;

impl Plugin for RulerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Ruler>().add_systems(Startup, spawn_pins).add_systems(Update, (follow_pins, aim, draw).chain());
    }
}

fn spawn_pins(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>, sites: Res<Sites>) {
    let mesh = meshes.add(Sphere::new(0.16));
    let material = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.9, 0.2), emissive: LinearRgba::rgb(4.0, 3.0, 0.4), ..default() });
    for (i, pin) in sites.pins.iter().enumerate() {
        commands.spawn((PinMark(i), Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()), Transform::from_translation(pin.position)));
    }
}

/// The marker of survey pin number `n`.
#[derive(Component)]
struct PinMark(usize);

/// Pins on things that move (the ledge launcher's release point, the light gate) move with them.
fn follow_pins(sites: Res<Sites>, mut marks: Query<(&PinMark, &mut Transform)>) {
    if !sites.is_changed() {
        return;
    }
    for (mark, mut transform) in &mut marks {
        if let Some(pin) = sites.pins.get(mark.0) {
            transform.translation = pin.position;
        }
    }
}

/// What the ruler would pick for a cursor ray.
pub fn pick_at(world: &VoxelWorld, sites: &Sites, camera: &Camera, camera_transform: &GlobalTransform, cursor: Vec2) -> Option<Picked> {
    let nearest_pin = sites
        .pins
        .iter()
        .filter(|pin| pin.present())
        .filter_map(|pin| {
            let on_screen = camera.world_to_viewport(camera_transform, pin.position).ok()?;
            let distance = on_screen.distance(cursor);
            (distance <= PIN_SNAP_PIXELS).then_some((distance, pin))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, pin)) = nearest_pin {
        return Some(Picked { point: pin.position, label: pin.label.to_string() });
    }
    let ray = camera.viewport_to_world(camera_transform, cursor).ok()?;
    let hit = world.raycast(ray.origin, *ray.direction, 300.0)?;
    let face_centre = hit.cell.as_vec3() + Vec3::splat(0.5) + hit.normal.as_vec3() * 0.5;
    let name = world.get(hit.cell).properties().name.to_lowercase();
    let face = if hit.normal == IVec3::Y { "top" } else { "side" };
    Some(Picked { point: face_centre, label: format!("{name} block, {face} face") })
}

fn aim(
    mut ruler: ResMut<Ruler>,
    world: Res<VoxelWorld>,
    sites: Res<Sites>,
    wants: Res<EguiWantsInput>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
) {
    if !ruler.active {
        return;
    }
    ruler.hover = None;
    if wants.wants_any_pointer_input() {
        return;
    }
    let Some(cursor) = window.cursor_position() else { return };
    let (camera, camera_transform) = *camera;
    ruler.hover = pick_at(&world, &sites, camera, camera_transform, cursor);
    if buttons.just_pressed(MouseButton::Left)
        && let Some(picked) = ruler.hover.clone()
    {
        ruler.pick(picked);
    }
}

fn draw(ruler: Res<Ruler>, mut gizmos: Gizmos) {
    if !ruler.in_use() {
        return;
    }
    let cyan = Color::srgb(0.2, 0.95, 1.0);
    let white = Color::WHITE;
    if let Some(hover) = &ruler.hover {
        gizmos.sphere(hover.point, 0.28, white);
    }
    if let Some(first) = &ruler.first {
        gizmos.sphere(first.point, 0.3, cyan);
        let end = ruler.second.as_ref().or(ruler.hover.as_ref().filter(|_| ruler.second.is_none()));
        if let Some(end) = end {
            // The direct line, and the horizontal and vertical legs it is made of.
            let corner = Vec3::new(end.point.x, first.point.y, end.point.z);
            gizmos.line(first.point, end.point, cyan);
            gizmos.line(first.point, corner, white);
            gizmos.line(corner, end.point, white);
            gizmos.sphere(end.point, 0.3, cyan);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picked(x: f32, y: f32, z: f32) -> Picked {
        Picked { point: Vec3::new(x, y, z), label: String::new() }
    }

    #[test]
    fn separation_is_split_into_horizontal_and_vertical() {
        let mut ruler = Ruler::default();
        ruler.start();
        ruler.pick(picked(70.0, 28.0, 56.5));
        assert_eq!(ruler.measurement(), None);
        assert!(ruler.active);
        ruler.pick(picked(91.5, 20.0, 56.5));
        let m = ruler.measurement().unwrap();
        assert_eq!((m.horizontal, m.vertical), (21.5, 8.0));
        assert!((m.direct - (21.5f32 * 21.5 + 64.0).sqrt()).abs() < 1e-5);
        // The second point ends the measurement; the reading stays until put away.
        assert!(!ruler.active && ruler.in_use());
        ruler.start();
        assert_eq!(ruler.measurement(), None);
        ruler.put_away();
        assert!(!ruler.in_use());
    }
}
