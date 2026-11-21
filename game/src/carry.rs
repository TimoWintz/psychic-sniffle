//! Carrying: pick something up, carry it, put it down or into the place a station reads.
//!
//! What has an orange handle (our paint) moves. The builders' dressed blocks are real blocks of the
//! island: they can be stood on and measured. Each block belongs to a yard (the channel ledge's pile, or the cart's block beside
//! the supply rail) and is put down only there. The light gate is not a block: it rests on a stand by
//! the drop tower's station until it is clamped on the tower.
//!
//! What a station reads is a [`Slot`]: the cart on the supply rail (its one block), the notches of
//! the drop tower (the light gate), the jack frame under the ledge launcher (a stack of blocks). `F`
//! facing a slot puts in what is carried, or takes it out again. What is in each slot is kept in the
//! save; loose blocks are not: they are back where they started the next time the game starts.

use crate::island::{Pin, Sites};
use crate::player::Player;
use crate::sim::Sim;
use crate::state::{Game, Mode, Ui};
use crate::voxel::VoxelWorld;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy_egui::input::EguiWantsInput;
use physics_core::materials::Material;
use physics_core::puzzle::{Ch1Layout, PuzzleKind};

/// How far in front of the avatar's feet a block is picked up or put down.
const AHEAD: f32 = 1.1;
/// How close to a slot's foot (or the gate's stand) the avatar must stand to use it.
const SLOT_REACH: f32 = 1.9;
const TOWER_REACH: f32 = 2.4;
const STAND_REACH: f32 = 1.6;
/// How close, in metres, the pointer's ray must pass to the drop tower's fall line to pick a notch.
const AIM_REACH: f32 = 1.5;
/// Where the carried block is held, in the avatar's frame (its front is −Z).
pub const HELD_AT: Vec3 = Vec3::new(0.0, 1.05, -0.62);
const HELD_SIZE: f32 = 0.8;
const HANDLE: Color = Color::srgb(1.0, 0.56, 0.10);
/// From the light gate's beam to the centre of the tower's column it clamps on.
const GATE_REACH: f32 = 2.0;
/// The light gate's size while it is carried or on its stand (it folds).
const GATE_FOLDED: f32 = 0.45;

/// The places loose blocks are put down. A block stays in its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Yard {
    /// The channel ledge, north of the launcher's rail: the pile of the builders' blocks.
    Ledge,
    /// Beside the supply rail's start: the cart's block.
    Rail,
}

/// What can be carried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Block(Yard),
    Gate,
}

/// How a slot holds what is put in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    /// Blocks stacked in a column, one metre each.
    Stack,
    /// One thing, or nothing.
    Single,
    /// One thing, clamped at a whole-metre notch along a column.
    Notches,
}

/// A place in the world that a station reads. What it holds is the save's `placed` value for its
/// puzzle: blocks in the frame, 0 or 1 on the cart, the gate's notch (its drop below the release
/// point, 0 for none).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// The cart on the supply rail.
    Cart,
    /// The drop tower's notches, for the light gate.
    Tower,
    /// The jack frame under the ledge launcher.
    Frame,
}

pub const SLOTS: [Slot; 3] = [Slot::Cart, Slot::Tower, Slot::Frame];

impl Slot {
    fn index(self) -> usize {
        self as usize
    }

    pub fn kind(self) -> SlotKind {
        match self {
            Slot::Cart => SlotKind::Single,
            Slot::Tower => SlotKind::Notches,
            Slot::Frame => SlotKind::Stack,
        }
    }

    /// The puzzle whose station reads it.
    pub fn puzzle(self) -> PuzzleKind {
        match self {
            Slot::Cart => PuzzleKind::Rail,
            Slot::Tower => PuzzleKind::Gravity,
            Slot::Frame => PuzzleKind::Channel,
        }
    }

    /// What goes in it.
    pub fn takes(self) -> Item {
        match self {
            Slot::Cart => Item::Block(Yard::Rail),
            Slot::Tower => Item::Gate,
            Slot::Frame => Item::Block(Yard::Ledge),
        }
    }

    /// The most it holds: blocks in the frame, the lowest notch on the tower.
    pub fn most(self, sites: &Sites) -> u32 {
        match self {
            Slot::Cart => 1,
            Slot::Tower => tower_height(sites),
            Slot::Frame => Ch1Layout::MAX_STACK,
        }
    }

    /// Where it stands on the ground: the avatar faces this to use it.
    fn foot(self, sites: &Sites) -> Vec3 {
        match self {
            Slot::Cart => sites.cart,
            Slot::Tower => sites.tower_plate,
            Slot::Frame => sites.channel_well.as_vec3() + Vec3::new(0.5, 0.0, 0.5),
        }
    }

    fn reach(self) -> f32 {
        if self == Slot::Tower { TOWER_REACH } else { SLOT_REACH }
    }
}

/// Height of the drop tower's release point above its landing plate, in whole metres.
fn tower_height(sites: &Sites) -> u32 {
    (sites.tower_release.y - sites.tower_plate.y).round() as u32
}

/// Where the light gate's beam crosses the fall line when it is clamped `notch` metres below the
/// release point.
pub fn gate_at(sites: &Sites, notch: u32) -> Vec3 {
    sites.tower_release - Vec3::Y * notch as f32
}

/// What `F` would do now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Pick up the loose block in this cell.
    Take(IVec3),
    /// Put the block down in this cell.
    Put(IVec3),
    /// Lift the light gate off its stand.
    TakeGate,
    /// Put the light gate back on its stand.
    StowGate,
    /// Put what is carried into a slot, which then holds this much.
    PutIn(Slot, u32),
    /// Take something out of a slot, which then holds this much.
    TakeOut(Slot, u32),
    /// Nothing can be done here, and why.
    Blocked(&'static str),
}

/// The movable things: loose blocks, what is in each slot, what is carried.
#[derive(Resource, Default)]
pub struct Carry {
    /// Cells of the blocks lying loose (not in a slot, not carried).
    pub loose: Vec<IVec3>,
    pub holding: Option<Item>,
    /// What each slot holds, by [`Slot`] order.
    pub filled: [u32; 3],
    /// What `F` would do, worked out each frame from where the avatar stands and faces.
    pub target: Option<Target>,
    /// The height on the drop tower's fall line that the pointer marks, when it is near the tower.
    pub aim: Option<f32>,
    /// Whether the rail cart has carried its block to the marker and gone on up the line, past the stone gate,
    /// to wait for the crawler: its slot can no longer be reached.
    pub cart_gone: bool,
}

impl Carry {
    pub fn holds(&self, slot: Slot) -> u32 {
        self.filled[slot.index()]
    }

    /// Whether the light gate is on its stand.
    pub fn gate_on_stand(&self) -> bool {
        self.holding != Some(Item::Gate) && self.holds(Slot::Tower) == 0
    }
}

/// Cells whose blocks changed since the terrain was last meshed; `scenery` rebuilds their chunks.
#[derive(Resource, Default)]
pub struct DirtyCells(pub Vec<IVec3>);

/// The orange handle and the physics engine's collider of one movable block.
#[derive(Component)]
struct BlockMark(usize);

/// The block in the avatar's hands.
#[derive(Component)]
struct HeldBlock;

/// The light gate, wherever it is.
#[derive(Component)]
struct Gate;

/// The light gate's stand.
#[derive(Component)]
struct GateStand;

pub struct CarryPlugin;

impl Plugin for CarryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Carry>().init_resource::<DirtyCells>().add_systems(
            Startup,
            lay_out.after(crate::scenery::spawn).after(crate::sim::setup).after(crate::rig::spawn),
        );
        app.add_systems(Update, (point, aim, act).chain().after(crate::player::move_player).run_if(in_state(Mode::Playing)))
            .add_systems(Update, (place_marks, hold, place_gate, draw_target));
    }
}

/// The yard a cell of loose block lies in.
fn yard_of(sites: &Sites, cell: IVec3) -> Yard {
    let [low, high] = sites.cargo_yard;
    if cell.cmpge(low).all() && cell.cmple(high).all() { Yard::Rail } else { Yard::Ledge }
}

fn yard_corners(sites: &Sites, yard: Yard) -> [IVec3; 2] {
    match yard {
        Yard::Ledge => sites.block_yard,
        Yard::Rail => sites.cargo_yard,
    }
}

/// Put everything where the save says: as many blocks in the jack frame as were left there, the
/// rest on the pile (the top layer leaves the pile first); the cart's block on the cart or beside the
/// rail; the light gate on its notch or its stand.
#[allow(clippy::too_many_arguments)]
fn lay_out(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut world: ResMut<VoxelWorld>,
    mut sites: ResMut<Sites>,
    mut carry: ResMut<Carry>,
    mut dirty: ResMut<DirtyCells>,
    game: Res<Game>,
) {
    let placed = |slot: Slot| {
        let puzzle = game.puzzle_of(slot.puzzle());
        game.placed(puzzle).min(slot.most(&sites))
    };
    let filled = SLOTS.map(placed);

    let stack = filled[Slot::Frame.index()];
    let mut pile = sites.block_pile.clone();
    // Bottom layer first, so the blocks taken away are the top ones.
    pile.sort_by_key(|c| (c.y, c.z, c.x));
    pile.truncate(pile.len() - stack as usize);
    if filled[Slot::Cart.index()] == 0 {
        pile.push(sites.cargo_cell);
    }
    for cell in &pile {
        world.set(*cell, Material::Limestone);
        dirty.0.push(*cell);
    }
    carry.loose = pile;
    for slot in SLOTS {
        set_slot(&mut world, &mut sites, &mut dirty, slot, filled[slot.index()]);
    }
    carry.filled = filled;

    let handle = meshes.add(Cuboid::new(0.62, 0.07, 0.14));
    let grip = meshes.add(Cuboid::new(0.5, 0.1, 0.05));
    let glow = materials.add(StandardMaterial { base_color: HANDLE, emissive: LinearRgba::rgb(2.0, 0.9, 0.1), ..default() });
    // One mark for every block that can lie loose or stand in the frame: the ledge's and the cargo.
    for i in 0..=Ch1Layout::MAX_STACK as usize {
        // A handle on top and a grip on each side, so blocks stacked in the frame read one by one.
        commands
            .spawn((
                BlockMark(i),
                RigidBody::Static,
                Collider::cuboid(1.0, 1.0, 1.0),
                Transform::from_translation(Vec3::NEG_Y * 100.0),
                Visibility::default(),
            ))
            .with_children(|c| {
                c.spawn((Mesh3d(handle.clone()), MeshMaterial3d(glow.clone()), Transform::from_xyz(0.0, 0.53, 0.0)));
                for turn in 0..4 {
                    let rotation = Quat::from_rotation_y(turn as f32 * std::f32::consts::FRAC_PI_2);
                    c.spawn((Mesh3d(grip.clone()), MeshMaterial3d(glow.clone()), Transform::from_translation(rotation * Vec3::new(0.0, 0.18, 0.515)).with_rotation(rotation)));
                }
            });
    }
    let body = meshes.add(Cuboid::from_length(HELD_SIZE));
    let stone = materials.add(StandardMaterial { base_color: colour(Material::Limestone), perceptual_roughness: 0.9, ..default() });
    commands.spawn((
        HeldBlock,
        Mesh3d(body),
        MeshMaterial3d(stone),
        Transform::default(),
        Visibility::Hidden,
        children![(Mesh3d(handle.clone()), MeshMaterial3d(glow.clone()), Transform::from_xyz(0.0, HELD_SIZE / 2.0 + 0.03, 0.0).with_scale(Vec3::splat(HELD_SIZE)))],
    ));

    // The light gate: two arms clamped on the tower's column either side of the fall line, an
    // emitter and a receiver facing each other across it, and an orange handle on the clamp. Its
    // origin is where the beam crosses the fall line.
    let dark = materials.add(StandardMaterial { base_color: Color::srgb(0.12, 0.13, 0.16), perceptual_roughness: 0.6, ..default() });
    let lens = materials.add(StandardMaterial { base_color: Color::srgb(1.0, 0.2, 0.15), emissive: LinearRgba::rgb(3.0, 0.3, 0.2), ..default() });
    let arm = meshes.add(Cuboid::new(2.2, 0.12, 0.12));
    let head = meshes.add(Cuboid::new(0.24, 0.24, 0.2));
    // The clamp wraps the tower's column, whose centre is two metres from the fall line.
    let clamp = meshes.add(Cuboid::new(1.2, 0.4, 1.6));
    let eye = meshes.add(Cuboid::new(0.1, 0.1, 0.04));
    commands
        .spawn((Gate, Transform::default(), Visibility::default()))
        .with_children(|g| {
            g.spawn((Mesh3d(clamp), MeshMaterial3d(dark.clone()), Transform::from_xyz(-GATE_REACH, 0.0, 0.0)));
            g.spawn((Mesh3d(handle.clone()), MeshMaterial3d(glow.clone()), Transform::from_xyz(-GATE_REACH, 0.24, 0.0)));
            for side in [-1.0f32, 1.0] {
                g.spawn((Mesh3d(arm.clone()), MeshMaterial3d(dark.clone()), Transform::from_xyz(-0.9, 0.0, side * 0.68)));
                g.spawn((Mesh3d(head.clone()), MeshMaterial3d(dark.clone()), Transform::from_xyz(0.0, 0.0, side * 0.68)));
                g.spawn((Mesh3d(eye.clone()), MeshMaterial3d(lens.clone()), Transform::from_xyz(0.0, 0.0, side * 0.56)));
            }
        });
    // Its stand: a post with a cradle the gate's clamp rests in.
    let post = meshes.add(Cuboid::new(0.14, 1.0, 0.14));
    let cradle = meshes.add(Cuboid::new(0.5, 0.08, 0.5));
    commands.spawn((GateStand, Transform::from_translation(sites.gate_stand), Visibility::default())).with_children(|s| {
        s.spawn((Mesh3d(post), MeshMaterial3d(dark.clone()), Transform::from_xyz(0.0, 0.5, 0.0)));
        s.spawn((Mesh3d(cradle), MeshMaterial3d(dark), Transform::from_xyz(0.0, 1.0, 0.0)));
    });
}

fn colour(material: Material) -> Color {
    let [r, g, b] = material.properties().colour;
    Color::srgb(r, g, b)
}

/// Make the world show what a slot holds: blocks in the jack frame (which raise the launcher and its
/// survey pin), the light gate's pin on its notch. The cart's cargo is shown by `sim`, which moves
/// the cart.
pub fn set_slot(world: &mut VoxelWorld, sites: &mut Sites, dirty: &mut DirtyCells, slot: Slot, holds: u32) {
    match slot {
        Slot::Frame => set_stack(world, sites, dirty, holds),
        Slot::Tower => {
            let pin = sites.gate_pin;
            sites.pins[pin].position = if holds == 0 { Pin::AWAY } else { gate_at(sites, holds) };
        }
        Slot::Cart => {}
    }
}

/// Fill the jack frame with `stack` blocks and raise the launcher (and its survey pin) onto them.
pub fn set_stack(world: &mut VoxelWorld, sites: &mut Sites, dirty: &mut DirtyCells, stack: u32) {
    for k in 0..=Ch1Layout::MAX_STACK as i32 {
        let cell = sites.channel_well + IVec3::Y * k;
        world.set(cell, if k < stack as i32 { Material::Limestone } else { Material::Air });
        dirty.0.push(cell);
    }
    sites.channel_launch.origin.y = (sites.channel_well.y + stack as i32) as f32;
    let pin = sites.channel_release_pin;
    sites.pins[pin].position = sites.channel_launch.origin;
}

/// Whether a cell may take a block of a yard that is put down: inside the yard, empty, on something
/// solid.
fn can_put(world: &VoxelWorld, sites: &Sites, yard: Yard, cell: IVec3) -> bool {
    let [low, high] = yard_corners(sites, yard);
    cell.cmpge(low).all() && cell.cmple(high).all() && !world.is_solid(cell) && world.is_solid(cell - IVec3::Y)
}

/// Whether the avatar's body overlaps a block cell.
fn overlaps_avatar(feet: Vec3, cell: IVec3) -> bool {
    let (low, high) = (cell.as_vec3(), cell.as_vec3() + Vec3::ONE);
    let (a, b) = (feet - Vec3::new(0.3, 0.0, 0.3), feet + Vec3::new(0.3, 1.8, 0.3));
    a.x < high.x && b.x > low.x && a.y < high.y && b.y > low.y && a.z < high.z && b.z > low.z
}

/// The notch the light gate goes to: the one nearest the height the pointer marks on the tower, or,
/// with the pointer elsewhere, the one at the avatar's hands.
pub fn notch_for(sites: &Sites, feet: Vec3, aim: Option<f32>) -> u32 {
    let height = aim.unwrap_or(feet.y + 1.0);
    let notch = (sites.tower_release.y - height).round().max(1.0) as u32;
    notch.min(tower_height(sites))
}

/// What `F` would do for an avatar at `feet` facing along `forward` (horizontal, unit length), with
/// the pointer marking `aim` on the drop tower (see [`Carry::aim`]).
pub fn target_for(world: &VoxelWorld, sites: &Sites, carry: &Carry, feet: Vec3, forward: Vec3, aim: Option<f32>) -> Option<Target> {
    let faces = |point: Vec3, reach: f32| {
        let to = (point - feet).with_y(0.0);
        to.length() < reach && (feet.y - point.y).abs() < 0.6 && to.normalize_or_zero().dot(forward) > 0.5
    };
    let at = |slot: Slot| faces(slot.foot(sites), slot.reach()) && !(slot == Slot::Cart && carry.cart_gone);
    let level = (feet.y + 0.01).floor() as i32;
    if let Some(item) = carry.holding {
        if let Some(slot) = SLOTS.into_iter().find(|s| s.takes() == item && at(*s)) {
            let holds = carry.holds(slot);
            return Some(match slot.kind() {
                SlotKind::Stack if holds < slot.most(sites) => Target::PutIn(slot, holds + 1),
                SlotKind::Stack => Target::Blocked("The jack frame is full."),
                SlotKind::Single => Target::PutIn(slot, 1),
                SlotKind::Notches => Target::PutIn(slot, notch_for(sites, feet, aim)),
            });
        }
        let yard = match item {
            Item::Gate if faces(sites.gate_stand, STAND_REACH) => return Some(Target::StowGate),
            Item::Gate => return Some(Target::Blocked("The light gate clamps on the drop tower, or goes back on its stand.")),
            Item::Block(yard) => yard,
        };
        // On the ground in front, or on top of a block there.
        let ahead = (feet + forward * AHEAD).floor().as_ivec3();
        for y in [level, level + 1] {
            let cell = IVec3::new(ahead.x, y, ahead.z);
            if can_put(world, sites, yard, cell) && !overlaps_avatar(feet, cell) {
                return Some(Target::Put(cell));
            }
        }
        let [low, high] = yard_corners(sites, yard);
        let in_yard = (low.x..=high.x).contains(&ahead.x) && (low.z..=high.z).contains(&ahead.z);
        return Some(Target::Blocked(match (in_yard, yard) {
            (true, _) => "No room to put the block down here.",
            (false, Yard::Ledge) => "Blocks go on the ledge, west of the launcher's rail.",
            (false, Yard::Rail) => "The cart's block goes on the cart, or down beside the start of the rail.",
        }));
    }
    if let Some(slot) = SLOTS.into_iter().find(|s| carry.holds(*s) > 0 && at(*s)) {
        let left = if slot.kind() == SlotKind::Stack { carry.holds(slot) - 1 } else { 0 };
        return Some(Target::TakeOut(slot, left));
    }
    if carry.gate_on_stand() && faces(sites.gate_stand, STAND_REACH) {
        return Some(Target::TakeGate);
    }
    // The nearest loose block in front with nothing on top of it, within reach of the hands.
    let reach_point = feet + forward * 0.9;
    carry
        .loose
        .iter()
        .filter(|c| !carry.loose.contains(&(**c + IVec3::Y)) && (level - 1..=level + 1).contains(&c.y))
        .map(|c| ((c.as_vec3() + Vec3::new(0.5, 0.0, 0.5) - reach_point).with_y(0.0).length(), *c))
        .filter(|(d, _)| *d < 1.2)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, c)| Target::Take(c))
}

/// Where the pointer marks the drop tower's fall line: the height on it nearest the pointer's ray,
/// if the ray passes close enough.
pub fn aim_on_tower(sites: &Sites, origin: Vec3, direction: Vec3) -> Option<f32> {
    let line = sites.tower_plate.with_y(0.0);
    // Closest approach between the ray and the vertical line through the plate.
    let flat = direction.with_y(0.0);
    let along = if flat.length_squared() < 1e-6 { 0.0 } else { (line - origin.with_y(0.0)).dot(flat) / flat.length_squared() };
    if along <= 0.0 {
        return None;
    }
    let near = origin + direction * along;
    let low = sites.tower_plate.y;
    ((near.with_y(0.0) - line).length() < AIM_REACH && (low - 0.5..=sites.tower_release.y + 0.5).contains(&near.y)).then_some(near.y)
}

/// Follow the pointer onto the drop tower while the light gate is carried.
fn point(
    mut carry: ResMut<Carry>,
    sites: Res<Sites>,
    wants: Res<EguiWantsInput>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
) {
    let aim = (carry.holding == Some(Item::Gate) && !wants.wants_any_pointer_input())
        .then(|| window.cursor_position())
        .flatten()
        .and_then(|cursor| camera.0.viewport_to_world(camera.1, cursor).ok())
        .and_then(|ray| aim_on_tower(&sites, ray.origin, *ray.direction));
    if carry.aim != aim {
        carry.aim = aim;
    }
}

#[allow(clippy::too_many_arguments)]
fn aim(
    world: Res<VoxelWorld>,
    sites: Res<Sites>,
    ui: Res<Ui>,
    sim: Res<Sim>,
    mut carry: ResMut<Carry>,
    player: Single<&Transform, With<Player>>,
) {
    let busy = ui.station.is_some() || sim.busy();
    let forward = (player.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
    let target = if busy { None } else { target_for(&world, &sites, &carry, player.translation, forward, carry.aim) };
    if carry.target != target {
        carry.target = target;
    }
}

#[allow(clippy::too_many_arguments)]
fn act(
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    mut world: ResMut<VoxelWorld>,
    mut sites: ResMut<Sites>,
    mut carry: ResMut<Carry>,
    mut dirty: ResMut<DirtyCells>,
    mut game: ResMut<Game>,
) {
    if wants.wants_any_keyboard_input() || !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Some(target) = carry.target else { return };
    apply(&mut world, &mut sites, &mut carry, &mut dirty, &mut game, target);
}

/// Do what a target says. Also used by the scripted tour.
pub fn apply(world: &mut VoxelWorld, sites: &mut Sites, carry: &mut Carry, dirty: &mut DirtyCells, game: &mut Game, target: Target) {
    match target {
        Target::Take(cell) => {
            let Some(i) = carry.loose.iter().position(|c| *c == cell) else { return };
            carry.loose.swap_remove(i);
            world.set(cell, Material::Air);
            dirty.0.push(cell);
            carry.holding = Some(Item::Block(yard_of(sites, cell)));
        }
        Target::Put(cell) => {
            world.set(cell, Material::Limestone);
            dirty.0.push(cell);
            carry.loose.push(cell);
            carry.holding = None;
        }
        Target::TakeGate => carry.holding = Some(Item::Gate),
        Target::StowGate => carry.holding = None,
        Target::PutIn(slot, holds) | Target::TakeOut(slot, holds) => {
            set_slot(world, sites, dirty, slot, holds);
            carry.filled[slot.index()] = holds;
            carry.holding = matches!(target, Target::TakeOut(..)).then_some(slot.takes());
            let puzzle = game.puzzle_of(slot.puzzle());
            let id = game.def(puzzle).id.clone();
            game.save.placed.insert(id, holds);
            game.persist();
        }
        Target::Blocked(_) => return,
    }
    carry.target = None;
}

/// Keep the handles and colliders on the blocks: the loose ones, then the ones in the frame.
fn place_marks(carry: Res<Carry>, sites: Res<Sites>, mut marks: Query<(&BlockMark, &mut Transform, &mut Visibility)>) {
    if !carry.is_changed() {
        return;
    }
    let stack = carry.holds(Slot::Frame) as i32;
    let cells: Vec<IVec3> = carry.loose.iter().copied().chain((0..stack).map(|k| sites.channel_well + IVec3::Y * k)).collect();
    for (mark, mut transform, mut visibility) in &mut marks {
        match cells.get(mark.0) {
            Some(cell) => {
                transform.translation = cell.as_vec3() + Vec3::splat(0.5);
                *visibility = Visibility::Inherited;
            }
            None => {
                transform.translation = Vec3::NEG_Y * 100.0;
                *visibility = Visibility::Hidden;
            }
        }
    }
}

/// A carried block rides in front of the avatar, between its hands.
fn hold(carry: Res<Carry>, player: Query<&Transform, (With<Player>, Without<HeldBlock>)>, mut held: Single<(&mut Transform, &mut Visibility), With<HeldBlock>>) {
    let (transform, visibility) = &mut *held;
    let Ok(avatar) = player.single() else { return };
    **visibility = if matches!(carry.holding, Some(Item::Block(_))) { Visibility::Inherited } else { Visibility::Hidden };
    transform.translation = avatar.translation + avatar.rotation * HELD_AT;
    transform.rotation = avatar.rotation;
}

/// The light gate: on its stand, in the avatar's hands, or clamped on its notch, its arms reaching
/// from the tower's column to either side of the fall line.
fn place_gate(carry: Res<Carry>, sites: Res<Sites>, player: Query<&Transform, (With<Player>, Without<Gate>)>, mut gate: Single<&mut Transform, With<Gate>>) {
    let folded = Vec3::splat(GATE_FOLDED);
    **gate = if let Some(Item::Gate) = carry.holding {
        let Ok(avatar) = player.single() else { return };
        // Carried by its clamp in both hands, arms pointing ahead.
        let rotation = avatar.rotation * Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let origin = HELD_AT + Vec3::NEG_Z * GATE_REACH * GATE_FOLDED;
        Transform::from_translation(avatar.translation + avatar.rotation * origin).with_rotation(rotation).with_scale(folded)
    } else if carry.holds(Slot::Tower) > 0 {
        Transform::from_translation(gate_at(&sites, carry.holds(Slot::Tower)))
    } else {
        // Its clamp resting in the stand's cradle.
        Transform::from_translation(sites.gate_stand + Vec3::new(GATE_REACH * GATE_FOLDED, 1.12, 0.0)).with_scale(folded)
    };
}

/// Outline where `F` would act: the block to lift, the ghost of the one to put down, the notch the
/// light gate would clamp at.
fn draw_target(carry: Res<Carry>, sites: Res<Sites>, mut gizmos: Gizmos) {
    let Some(target) = carry.target else { return };
    let take = Color::srgb(1.0, 0.85, 0.3);
    let put = Color::srgb(0.4, 0.9, 1.0);
    let cube = |gizmos: &mut Gizmos, centre: Vec3, size: Vec3, colour: Color| gizmos.cube(Transform::from_translation(centre).with_scale(size), colour);
    match target {
        Target::Take(cell) => cube(&mut gizmos, cell.as_vec3() + Vec3::splat(0.5), Vec3::splat(1.04), take),
        Target::Put(cell) => cube(&mut gizmos, cell.as_vec3() + Vec3::splat(0.5), Vec3::splat(1.04), put),
        Target::PutIn(Slot::Frame, holds) => cube(&mut gizmos, (sites.channel_well + IVec3::Y * (holds as i32 - 1)).as_vec3() + Vec3::splat(0.5), Vec3::splat(1.04), put),
        Target::TakeOut(Slot::Frame, left) => cube(&mut gizmos, (sites.channel_well + IVec3::Y * left as i32).as_vec3() + Vec3::splat(0.5), Vec3::splat(1.04), take),
        Target::PutIn(Slot::Cart, _) => cube(&mut gizmos, sites.cart + Vec3::Y * (0.7 + HELD_SIZE / 2.0), Vec3::splat(HELD_SIZE + 0.04), put),
        Target::TakeOut(Slot::Cart, _) => cube(&mut gizmos, sites.cart + Vec3::Y * (0.7 + HELD_SIZE / 2.0), Vec3::splat(HELD_SIZE + 0.04), take),
        Target::PutIn(Slot::Tower, notch) => {
            // The ghost of the gate at the notch: its beam across the fall line, and a tick on the
            // column at every notch so the choice reads as one of them.
            let at = gate_at(&sites, notch);
            gizmos.line(at - Vec3::Z * 0.7, at + Vec3::Z * 0.7, put);
            cube(&mut gizmos, at - Vec3::X * 0.9, Vec3::new(2.4, 0.2, 1.5), put);
            let column = sites.tower_release - Vec3::X * GATE_REACH;
            for n in 1..=tower_height(&sites) {
                let tick = column - Vec3::Y * n as f32;
                gizmos.line(tick + Vec3::X * 0.5, tick + Vec3::X * if n == notch { 1.0 } else { 0.7 }, put);
            }
        }
        Target::TakeOut(Slot::Tower, _) => {
            cube(&mut gizmos, gate_at(&sites, carry.holds(Slot::Tower)) - Vec3::X * 0.9, Vec3::new(2.4, 0.3, 1.6), take);
        }
        Target::TakeGate => cube(&mut gizmos, sites.gate_stand + Vec3::new(0.5, 1.12, 0.0), Vec3::new(1.4, 0.3, 0.8), take),
        Target::StowGate => cube(&mut gizmos, sites.gate_stand + Vec3::new(0.5, 1.12, 0.0), Vec3::new(1.4, 0.3, 0.8), put),
        Target::Blocked(_) => {}
    }
}

/// The line the hints bar shows for what `F` would do.
pub fn hint(carry: &Carry) -> Option<String> {
    Some(match carry.target? {
        Target::Take(_) => "F: pick up the block".into(),
        Target::Put(_) => "F: put the block down".into(),
        Target::TakeGate => "F: take the light gate off its stand".into(),
        Target::StowGate => "F: put the light gate back on its stand".into(),
        Target::PutIn(Slot::Frame, _) => "F: slide the block into the jack frame (the launcher goes up a metre)".into(),
        Target::TakeOut(Slot::Frame, _) => "F: take a block out of the jack frame (the launcher comes down a metre)".into(),
        Target::PutIn(Slot::Cart, _) => "F: load the block onto the cart".into(),
        Target::TakeOut(Slot::Cart, _) => "F: take the block off the cart".into(),
        Target::PutIn(Slot::Tower, notch) => {
            format!("F: clamp the gate {notch} m below the release point (point higher or lower on the tower to choose the notch)")
        }
        Target::TakeOut(Slot::Tower, _) => "F: unclamp the light gate".into(),
        Target::Blocked(why) => format!("{} {why}", if carry.holding == Some(Item::Gate) { "Carrying the light gate." } else { "Carrying a block." }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::island;

    fn laid_out(seed: u64) -> (VoxelWorld, Sites, Carry) {
        let (mut world, sites) = island::build(&Ch1Layout::generate(seed));
        let mut loose = sites.block_pile.clone();
        loose.push(sites.cargo_cell);
        for cell in &loose {
            world.set(*cell, Material::Limestone);
        }
        let carry = Carry { loose, ..default() };
        (world, sites, carry)
    }

    #[test]
    fn a_block_from_the_pile_goes_into_the_frame_and_raises_the_launcher() {
        for seed in [0, 7, 42] {
            let (mut world, mut sites, mut carry) = laid_out(seed);
            let base = sites.channel_launch.origin;
            let mut dirty = DirtyCells::default();

            // Facing the pile from the yard's west side: the near block on top comes first.
            let pile_west = sites.block_pile.iter().map(|c| c.x).min().unwrap();
            let row = sites.block_pile[0].z;
            let feet = Vec3::new(pile_west as f32 - 0.6, base.y, row as f32 + 0.5);
            let Some(Target::Take(cell)) = target_for(&world, &sites, &carry, feet, Vec3::X, None) else { panic!("seed {seed}: nothing to take") };
            assert_eq!(cell.y, base.y as i32 + 1, "the top block is taken first");
            assert_eq!(yard_of(&sites, cell), Yard::Ledge);
            world.set(cell, Material::Air);
            carry.loose.retain(|c| *c != cell);
            carry.holding = Some(Item::Block(Yard::Ledge));

            // At the frame's foot on the launcher's rail, facing it.
            let out = sites.channel_launch.direction;
            let foot = sites.channel_well.as_vec3() + Vec3::new(0.5, 0.0, 0.5) - out * 1.4;
            assert_eq!(target_for(&world, &sites, &carry, foot, out, None), Some(Target::PutIn(Slot::Frame, 1)), "seed {seed}");
            // Facing away from it, the block cannot be put on the rail.
            assert!(matches!(target_for(&world, &sites, &carry, foot, -out, None), Some(Target::Blocked(_))), "seed {seed}");

            set_stack(&mut world, &mut sites, &mut dirty, 3);
            assert_eq!(sites.channel_launch.origin, base + Vec3::Y * 3.0);
            assert_eq!(sites.pins[sites.channel_release_pin].position, sites.channel_launch.origin);
            assert!(world.is_solid(sites.channel_well + IVec3::Y * 2));
            assert!(!world.is_solid(sites.channel_well + IVec3::Y * 3));
            carry.filled[Slot::Frame.index()] = 3;
            carry.holding = None;
            assert_eq!(target_for(&world, &sites, &carry, foot, out, None), Some(Target::TakeOut(Slot::Frame, 2)));
            set_stack(&mut world, &mut sites, &mut dirty, 0);
            assert_eq!(sites.channel_launch.origin, base);
        }
    }

    #[test]
    fn blocks_are_put_down_only_in_their_yard_and_never_on_the_avatar() {
        let (world, sites, mut carry) = laid_out(3);
        carry.holding = Some(Item::Block(Yard::Ledge));
        // At the yard's west edge, clear of the pile.
        let [low, high] = sites.block_yard;
        let feet = Vec3::new(low.x as f32 + 0.5, low.y as f32, high.z as f32 - 1.5);
        assert_eq!(target_for(&world, &sites, &carry, feet, Vec3::Z, None), Some(Target::Put(IVec3::new(low.x, low.y, high.z - 1))));
        // Facing out of the yard (west, off its edge).
        assert!(matches!(target_for(&world, &sites, &carry, feet, Vec3::NEG_X, None), Some(Target::Blocked(_))));
        assert!(overlaps_avatar(feet, feet.floor().as_ivec3()));
        // A ledge block does not go into the rail's yard, nor onto the cart.
        let [rail_low, _] = sites.cargo_yard;
        let by_rail = Vec3::new(rail_low.x as f32 + 0.5, rail_low.y as f32, rail_low.z as f32 - 0.6);
        assert!(matches!(target_for(&world, &sites, &carry, by_rail, Vec3::Z, None), Some(Target::Blocked(_))));
    }

    #[test]
    fn the_carts_block_is_loaded_onto_the_cart() {
        for seed in [0, 7, 42, 199] {
            let (mut world, sites, mut carry) = laid_out(seed);
            // From beside the block, facing it.
            let cell = sites.cargo_cell;
            let feet = Vec3::new(cell.x as f32 - 0.6, cell.y as f32, cell.z as f32 + 0.5);
            assert_eq!(target_for(&world, &sites, &carry, feet, Vec3::X, None), Some(Target::Take(cell)), "seed {seed}");
            world.set(cell, Material::Air);
            carry.loose.retain(|c| *c != cell);
            carry.holding = Some(Item::Block(yard_of(&sites, cell)));
            assert_eq!(carry.holding, Some(Item::Block(Yard::Rail)));
            // Facing the cart from the rail behind it.
            let behind = sites.cart - Vec3::X * 1.2;
            assert_eq!(target_for(&world, &sites, &carry, behind, Vec3::X, None), Some(Target::PutIn(Slot::Cart, 1)), "seed {seed}");
            // Back down in its yard instead.
            assert_eq!(target_for(&world, &sites, &carry, feet, Vec3::X, None), Some(Target::Put(cell)), "seed {seed}");
            // A block on the cart comes off it again.
            carry.holding = None;
            carry.filled[Slot::Cart.index()] = 1;
            assert_eq!(target_for(&world, &sites, &carry, behind, Vec3::X, None), Some(Target::TakeOut(Slot::Cart, 0)));
        }
    }

    #[test]
    fn the_light_gate_is_clamped_at_the_notch_pointed_at() {
        for seed in [0, 7, 42, 199] {
            let layout = Ch1Layout::generate(seed);
            let (world, mut sites, mut carry) = laid_out(seed);
            let mut dirty = DirtyCells::default();
            let mut world = world;
            // Off its stand.
            let at_stand = sites.gate_stand - Vec3::X * 1.0;
            assert_eq!(target_for(&world, &sites, &carry, at_stand, Vec3::X, None), Some(Target::TakeGate), "seed {seed}");
            carry.holding = Some(Item::Gate);
            assert_eq!(target_for(&world, &sites, &carry, at_stand, Vec3::X, None), Some(Target::StowGate));
            // At the foot of the tower, facing the fall line: the notch pointed at, else hand height.
            let feet = sites.tower_plate + Vec3::Z * 1.8;
            let toward = Vec3::NEG_Z;
            assert_eq!(target_for(&world, &sites, &carry, feet, toward, None), Some(Target::PutIn(Slot::Tower, layout.tower_h - 1)));
            let five_below = sites.tower_release.y - 5.2;
            assert_eq!(target_for(&world, &sites, &carry, feet, toward, Some(five_below)), Some(Target::PutIn(Slot::Tower, 5)));
            // Notches run from a metre below the release to the plate.
            assert_eq!(notch_for(&sites, feet, Some(sites.tower_release.y + 3.0)), 1);
            assert_eq!(notch_for(&sites, feet, Some(sites.tower_plate.y - 3.0)), layout.tower_h);
            // Clamped, its pin is on the fall line that far below the release point.
            set_slot(&mut world, &mut sites, &mut dirty, Slot::Tower, 5);
            let pin = &sites.pins[sites.gate_pin];
            assert!(pin.present());
            assert_eq!(sites.tower_release.y - pin.position.y, 5.0);
            assert_eq!(pin.position.with_y(0.0), sites.tower_release.with_y(0.0));
            carry.holding = None;
            carry.filled[Slot::Tower.index()] = 5;
            assert_eq!(target_for(&world, &sites, &carry, feet, toward, None), Some(Target::TakeOut(Slot::Tower, 0)));
            assert!(!carry.gate_on_stand());
            set_slot(&mut world, &mut sites, &mut dirty, Slot::Tower, 0);
            assert!(!sites.pins[sites.gate_pin].present());
        }
    }

    #[test]
    fn the_pointer_marks_a_height_on_the_tower() {
        let (_, sites, _) = laid_out(5);
        let target = sites.tower_plate + Vec3::Y * 6.0;
        let eye = target + Vec3::new(-8.0, 2.0, 9.0);
        let aim = aim_on_tower(&sites, eye, (target - eye).normalize()).expect("the ray passes the tower");
        assert!((aim - target.y).abs() < 1e-3, "{aim}");
        // Looking away from the tower marks nothing.
        assert_eq!(aim_on_tower(&sites, eye, (eye - target).normalize()), None);
        // Nor does a ray that passes far to the side.
        assert_eq!(aim_on_tower(&sites, eye, (target + Vec3::Z * 5.0 - eye).normalize()), None);
    }
}
