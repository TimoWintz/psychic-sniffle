//! Carrying: pick something up, carry it, put it down or into the place a station reads.
//!
//! What has an orange handle (our paint) moves. The builders' dressed blocks are real blocks of the
//! island: they can be stood on and measured. Each block belongs to a yard (the channel ledge's pile,
//! the cart's block beside the supply rail, or the beam's pile) and is put down only there. Beside
//! the sea-cliff quarry console sits a stack of smaller quarry stones: carry one to either orange
//! drop lip and press `F` to release it straight down while Lumen times the fall.
//!
//! What a station reads is a [`Slot`]: the cart on the supply rail (its one block) and the jack
//! frame under the ledge launcher (a stack of blocks). `F` facing a slot puts in what is carried,
//! or takes it out again. The beam, the side-machine off the road, takes its own blocks on the
//! seats of its two arms the same way. What is in each slot and on the beam is kept in the save;
//! loose blocks are not: they are back where they started the next time the game starts.

use crate::island::Sites;
use crate::player::Player;
use crate::sim::{Request, Sim};
use crate::state::{BuildWorld, Game, Mode, Ui, WorldEntity, not_paused};
use crate::voxel::VoxelWorld;
use avian3d::prelude::*;
use bevy::prelude::*;
use bevy_egui::input::EguiWantsInput;
use physics_core::beam::{Arm, MOST_ON_A_SEAT, PILE, Placement, SEATS};
use physics_core::materials::Material;
use physics_core::puzzle::{Ch1Layout, PuzzleKind};

/// How far in front of the avatar's feet a block is picked up or put down.
const AHEAD: f32 = 1.1;
/// How close to a slot's foot, drop lip, or stone stack the avatar must stand to use it.
const SLOT_REACH: f32 = 1.9;
const DROP_REACH: f32 = 2.0;
const PILE_REACH: f32 = 2.0;
/// Where the carried block is held, in the avatar's frame (its front is −Z).
pub const HELD_AT: Vec3 = Vec3::new(0.0, 1.05, -0.62);
const HELD_SIZE: f32 = 0.8;
/// Size of a loose quarry drop-stone (smaller than a 1 m structural block so a 1 m drop reads
/// clearly against its own height).
pub const QUARRY_STONE_SIZE: f32 = 0.5;
const HANDLE: Color = Color::srgb(1.0, 0.56, 0.10);

/// The places loose blocks are put down. A block stays in its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Yard {
    /// The channel ledge, north of the launcher's rail: the pile of the builders' blocks.
    Ledge,
    /// Beside the supply rail's start: the cart's block.
    Rail,
    /// Around the beam's pile, off the road: the beam's blocks.
    Beam,
}

/// Which quarry ledge a drop-stone is released from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropLedge {
    /// The 1 m step right beside the console.
    Low,
    /// The deep sea-cliff rim straight above the cove water.
    Cliff,
}

impl DropLedge {
    pub fn release(self, sites: &Sites) -> Vec3 {
        match self {
            DropLedge::Low => sites.low_drop,
            DropLedge::Cliff => sites.cliff_drop,
        }
    }

    pub fn landing(self, sites: &Sites) -> Vec3 {
        match self {
            DropLedge::Low => sites.low_landing,
            DropLedge::Cliff => sites.cliff_landing,
        }
    }
}

/// What can be carried.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Block(Yard),
    QuarryStone,
}

/// How a slot holds what is put in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotKind {
    /// Blocks stacked in a column, one metre each.
    Stack,
    /// One thing, or nothing.
    Single,
}

/// A place in the world that a station reads. What it holds is the save's `placed` value for its
/// puzzle: blocks in the frame, or 0/1 on the cart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slot {
    /// The cart on the supply rail.
    Cart,
    /// The jack frame under the ledge launcher.
    Frame,
}

pub const SLOTS: [Slot; 2] = [Slot::Cart, Slot::Frame];

impl Slot {
    fn index(self) -> usize {
        self as usize
    }

    pub fn kind(self) -> SlotKind {
        match self {
            Slot::Cart => SlotKind::Single,
            Slot::Frame => SlotKind::Stack,
        }
    }

    /// The puzzle whose station reads it.
    pub fn puzzle(self) -> PuzzleKind {
        match self {
            Slot::Cart => PuzzleKind::Rail,
            Slot::Frame => PuzzleKind::Channel,
        }
    }

    /// What goes in it.
    pub fn takes(self) -> Item {
        match self {
            Slot::Cart => Item::Block(Yard::Rail),
            Slot::Frame => Item::Block(Yard::Ledge),
        }
    }

    /// The most it holds: blocks in the frame, or 1 on the cart.
    pub fn most(self, _sites: &Sites) -> u32 {
        match self {
            Slot::Cart => 1,
            Slot::Frame => Ch1Layout::MAX_STACK,
        }
    }

    /// Where it stands on the ground: the avatar faces this to use it.
    fn foot(self, sites: &Sites) -> Vec3 {
        match self {
            Slot::Cart => sites.cart,
            Slot::Frame => sites.channel_well.as_vec3() + Vec3::new(0.5, 0.0, 0.5),
        }
    }
}

/// What `F` would do now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Pick up the loose block in this cell.
    Take(IVec3),
    /// Put the block down in this cell.
    Put(IVec3),
    /// Pick up a quarry drop-stone from the stack beside the console.
    TakeStone,
    /// Put the carried quarry drop-stone back on the stack.
    StowStone,
    /// Release the carried quarry drop-stone straight down from rest off a quarry ledge.
    DropStone(DropLedge),
    /// Put what is carried into a slot, which then holds this much.
    PutIn(Slot, u32),
    /// Take something out of a slot, which then holds this much.
    TakeOut(Slot, u32),
    /// Put the block on a seat of the beam.
    PutOnBeam(Arm, u32),
    /// Take the top block off a seat of the beam.
    TakeOffBeam(Arm, u32),
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
    pub filled: [u32; 2],
    /// What `F` would do, worked out each frame from where the avatar stands and faces.
    pub target: Option<Target>,
    /// Whether the rail cart has carried its block to the marker and gone on up the line, past the stone gate,
    /// to wait for the crawler: its slot can no longer be reached.
    pub cart_gone: bool,
    /// The player's blocks on the beam's seats.
    pub beam: Placement,
    /// Whether the beam is swinging, released, so nothing can be put on it or taken off.
    pub beam_swinging: bool,
    /// Floating world-space badge for the current placement target: `(viewport_px, label, is_put)`.
    pub badge: Option<(Vec2, String, bool)>,
}

impl Carry {
    pub fn holds(&self, slot: Slot) -> u32 {
        self.filled[slot.index()]
    }
}

/// Cells whose blocks changed since the terrain was last meshed; `scenery` rebuilds their chunks.
#[derive(Resource, Default)]
pub struct DirtyCells(pub Vec<IVec3>);

/// The orange handle and the physics engine's collider of one movable block.
#[derive(Component)]
struct BlockMark(usize);

/// The 1 m dressed block in the avatar's hands.
#[derive(Component)]
struct HeldBlock;

/// The smaller quarry drop-stone in the avatar's hands.
#[derive(Component)]
struct HeldStone;

pub struct CarryPlugin;

impl Plugin for CarryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Carry>()
            .init_resource::<DirtyCells>()
            .add_systems(BuildWorld, lay_out.after(crate::scenery::spawn).after(crate::sim::setup).after(crate::rig::spawn));
        app.add_systems(Update, (aim, act).chain().after(crate::player::move_player).run_if(in_state(Mode::Playing)).run_if(not_paused))
            .add_systems(Update, (place_marks, hold, draw_target));
    }
}

/// The yard a cell of loose block lies in.
fn yard_of(sites: &Sites, cell: IVec3) -> Yard {
    let inside = |[low, high]: [IVec3; 2]| cell.cmpge(low).all() && cell.cmple(high).all();
    if inside(sites.cargo_yard) {
        Yard::Rail
    } else if inside(sites.beam.yard) {
        Yard::Beam
    } else {
        Yard::Ledge
    }
}

fn yard_corners(sites: &Sites, yard: Yard) -> [IVec3; 2] {
    match yard {
        Yard::Ledge => sites.block_yard,
        Yard::Rail => sites.cargo_yard,
        Yard::Beam => sites.beam.yard,
    }
}

/// How many blocks of its own each yard's pile holds at the start, so how many marks there are.
const MARKS: usize = Ch1Layout::MAX_STACK as usize + 1 + PILE as usize;

/// Put everything where the save says: as many blocks in the jack frame as were left there, the
/// rest on the pile (the top layer leaves the pile first); the cart's block on the cart or beside the
/// rail; the beam's blocks on its seats, the rest on its pile; and the stack of quarry drop-stones
/// beside the sea-cliff console.
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
    *carry = Carry::default();
    dirty.0.clear();
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
    let on_beam = Placement::from_saved(&game.save.side(crate::beam::BEAM_ID).placed);
    let mut beam_pile = sites.beam.pile.clone();
    beam_pile.sort_by_key(|c| (c.y, c.z, c.x));
    beam_pile.truncate(beam_pile.len().saturating_sub(on_beam.blocks() as usize));
    pile.extend(beam_pile);
    carry.beam = on_beam;
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
    let glow = materials.add(StandardMaterial { base_color: HANDLE, emissive: LinearRgba::rgb(0.5, 0.2, 0.02), ..default() });
    // One mark for every block that can lie loose or stand in the frame: the ledge's, the cargo and
    // the beam's.
    for i in 0..MARKS {
        // A handle on top and a grip on each side, so blocks stacked in the frame read one by one.
        commands
            .spawn((
                WorldEntity,
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
                    c.spawn((
                        Mesh3d(grip.clone()),
                        MeshMaterial3d(glow.clone()),
                        Transform::from_translation(rotation * Vec3::new(0.0, 0.18, 0.515)).with_rotation(rotation),
                    ));
                }
            });
    }
    let body = meshes.add(Cuboid::from_length(HELD_SIZE));
    let stone = materials.add(StandardMaterial { base_color: colour(Material::Limestone), perceptual_roughness: 0.9, ..default() });
    commands.spawn((
        WorldEntity,
        HeldBlock,
        Mesh3d(body),
        MeshMaterial3d(stone.clone()),
        Transform::default(),
        Visibility::Hidden,
        children![(
            Mesh3d(handle.clone()),
            MeshMaterial3d(glow.clone()),
            Transform::from_xyz(0.0, HELD_SIZE / 2.0 + 0.03, 0.0).with_scale(Vec3::splat(HELD_SIZE))
        )],
    ));

    // The smaller quarry drop-stone in the avatar's hands.
    let small_stone = meshes.add(Cuboid::from_length(QUARRY_STONE_SIZE));
    let small_handle = meshes.add(Cuboid::new(0.36, 0.05, 0.09));
    commands.spawn((
        WorldEntity,
        HeldStone,
        Mesh3d(small_stone.clone()),
        MeshMaterial3d(stone.clone()),
        Transform::default(),
        Visibility::Hidden,
        children![(Mesh3d(small_handle.clone()), MeshMaterial3d(glow.clone()), Transform::from_xyz(0.0, QUARRY_STONE_SIZE / 2.0 + 0.025, 0.0),)],
    ));

    // A neat stack of quarry drop-stones beside the sea-cliff console: always ready to pick up
    // and drop off either ledge.
    let plinth = meshes.add(Cuboid::new(1.15, 0.14, 1.15));
    commands.spawn((WorldEntity, Transform::from_translation(sites.stone_pile), Visibility::default())).with_children(|s| {
        s.spawn((Mesh3d(plinth), MeshMaterial3d(stone.clone()), Transform::from_xyz(0.0, 0.07, 0.0)));
        for (x, y, z, yaw, has_handle) in [
            (-0.27, 0.14 + QUARRY_STONE_SIZE / 2.0, -0.22, 0.08, false),
            (0.27, 0.14 + QUARRY_STONE_SIZE / 2.0, -0.18, -0.1, false),
            (-0.18, 0.14 + QUARRY_STONE_SIZE / 2.0, 0.28, 0.14, false),
            (0.0, 0.14 + QUARRY_STONE_SIZE * 1.5, -0.02, -0.06, true),
        ] {
            let mut block =
                s.spawn((Mesh3d(small_stone.clone()), MeshMaterial3d(stone.clone()), Transform::from_xyz(x, y, z).with_rotation(Quat::from_rotation_y(yaw))));
            if has_handle {
                block.with_child((Mesh3d(small_handle.clone()), MeshMaterial3d(glow.clone()), Transform::from_xyz(0.0, QUARRY_STONE_SIZE / 2.0 + 0.025, 0.0)));
            }
        }
    });
}

fn colour(material: Material) -> Color {
    let [r, g, b] = material.properties().colour;
    Color::srgb(r, g, b)
}

/// Make the world show what a slot holds: blocks in the jack frame (which raise the launcher and its
/// survey pin). The cart's cargo is shown by `sim`, which moves the cart.
pub fn set_slot(world: &mut VoxelWorld, sites: &mut Sites, dirty: &mut DirtyCells, slot: Slot, holds: u32) {
    match slot {
        Slot::Frame => set_stack(world, sites, dirty, holds),
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

/// What `F` would do for an avatar at `feet` facing along `forward` (horizontal, unit length).
pub fn target_for(world: &VoxelWorld, sites: &Sites, carry: &Carry, feet: Vec3, forward: Vec3) -> Option<Target> {
    let faces = |point: Vec3, reach: f32| {
        let to = (point - feet).with_y(0.0);
        to.length() < reach && (feet.y - point.y).abs() < 0.6 && to.normalize_or_zero().dot(forward) > 0.45
    };
    let at = |slot: Slot| faces(slot.foot(sites), SLOT_REACH) && !(slot == Slot::Cart && carry.cart_gone);
    let level = (feet.y + 0.01).floor() as i32;
    let seat = beam_seat(sites, feet, forward);
    if let Some(item) = carry.holding {
        if item == Item::QuarryStone {
            for ledge in [DropLedge::Low, DropLedge::Cliff] {
                if faces(ledge.release(sites), DROP_REACH) {
                    return Some(Target::DropStone(ledge));
                }
            }
            if faces(sites.stone_pile, PILE_REACH) {
                return Some(Target::StowStone);
            }
            return Some(Target::Blocked("Carry the quarry stone to either orange drop lip and press F, or put it back on the stack."));
        }
        if let (Some(seat), Item::Block(Yard::Beam)) = (seat, item) {
            return Some(match seat {
                _ if carry.beam_swinging => Target::Blocked("Wait for the beam to come back level."),
                None => Target::Blocked("That's the pivot. Put the block on a seat of either arm."),
                Some((arm, seat)) if carry.beam.get(arm, seat) >= MOST_ON_A_SEAT => Target::Blocked("That seat is full."),
                Some((arm, seat)) => Target::PutOnBeam(arm, seat),
            });
        }
        if let Some(slot) = SLOTS.into_iter().find(|s| s.takes() == item && at(*s)) {
            let holds = carry.holds(slot);
            return Some(match slot.kind() {
                SlotKind::Stack if holds < slot.most(sites) => Target::PutIn(slot, holds + 1),
                SlotKind::Stack => Target::Blocked("The jack frame is full."),
                SlotKind::Single => Target::PutIn(slot, 1),
            });
        }
        let Item::Block(yard) = item else { unreachable!() };
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
            (false, Yard::Beam) => "The beam's blocks go on the beam, or down by their pile.",
        }));
    }
    if let Some(Some((arm, seat))) = seat
        && carry.beam.get(arm, seat) > 0
    {
        return Some(if carry.beam_swinging { Target::Blocked("Wait for the beam to come back level.") } else { Target::TakeOffBeam(arm, seat) });
    }
    if let Some(slot) = SLOTS.into_iter().find(|s| carry.holds(*s) > 0 && at(*s)) {
        let left = if slot.kind() == SlotKind::Stack { carry.holds(slot) - 1 } else { 0 };
        return Some(Target::TakeOut(slot, left));
    }
    if faces(sites.stone_pile, PILE_REACH) {
        return Some(Target::TakeStone);
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

/// The beam's seat the avatar faces, if it faces the beam: `Some(None)` for the pivot, where nothing
/// goes.
pub fn beam_seat(sites: &Sites, feet: Vec3, forward: Vec3) -> Option<Option<(Arm, u32)>> {
    let beam = &sites.beam;
    let ahead = feet + forward * AHEAD;
    let row = beam.row() as f32 + 0.5;
    let beside = (feet.z - row).abs();
    let along = ahead.x - beam.pivot.x;
    let facing = forward.z * (row - feet.z) > 0.0;
    let level = (feet.y - (beam.pivot.y - crate::island::BEAM_AXLE)).abs() < 0.6;
    if !(level && facing && beside < 2.2 && (ahead.z - row).abs() < 0.9 && along.abs() < SEATS as f32 + 0.5) {
        return None;
    }
    let offset = along.round() as i32;
    let arm = if offset < 0 { Arm::Load } else { Arm::Far };
    Some((offset != 0).then_some((arm, offset.unsigned_abs())))
}

/// World position and compact label for a placement ghost (quarry drop lip, jack frame height,
/// beam seat), plus whether the action places/drops (`true`) or removes (`false`).
pub fn badge_for(carry: &Carry, sites: &Sites) -> Option<(Vec3, String, bool)> {
    match carry.target? {
        Target::DropStone(DropLedge::Low) => Some((sites.low_drop + Vec3::Y * 0.65, "Drop stone (low step)".into(), true)),
        Target::DropStone(DropLedge::Cliff) => Some((sites.cliff_drop + Vec3::Y * 0.65, "Drop stone (cliff rim)".into(), true)),
        Target::PutIn(Slot::Frame, holds) => {
            let at = (sites.channel_well + IVec3::Y * (holds as i32 - 1)).as_vec3() + Vec3::new(0.5, 1.1, 0.5);
            let plural = if holds == 1 { "" } else { "s" };
            Some((at, format!("{holds} m ({holds} block{plural})"), true))
        }
        Target::TakeOut(Slot::Frame, left) => {
            let cur = carry.holds(Slot::Frame);
            let at = (sites.channel_well + IVec3::Y * left as i32).as_vec3() + Vec3::new(0.5, 1.1, 0.5);
            Some((at, format!("Stack: {cur} m → {left} m"), false))
        }
        Target::PutOnBeam(arm, seat) => {
            let count = carry.beam.get(arm, seat) + 1;
            let at = sites.beam.seat(arm, seat) + Vec3::Y * (HELD_SIZE * count as f32 + 0.35);
            Some((at, format!("Seat {seat} ({count})"), true))
        }
        Target::TakeOffBeam(arm, seat) => {
            let count = carry.beam.get(arm, seat);
            let at = sites.beam.seat(arm, seat) + Vec3::Y * (HELD_SIZE * count as f32 + 0.35);
            Some((at, format!("Seat {seat} ({count})"), false))
        }
        _ => None,
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
    camera: Single<(&Camera, &GlobalTransform), With<Camera3d>>,
) {
    let busy = ui.at_console() || sim.busy();
    let forward = (player.rotation * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
    let target = if busy { None } else { target_for(&world, &sites, &carry, player.translation, forward) };
    if carry.target != target {
        carry.target = target;
    }
    carry.badge = badge_for(&carry, &sites).and_then(|(at, text, is_put)| {
        let screen = camera.0.world_to_viewport(camera.1, at).ok()?;
        Some((screen, text, is_put))
    });
}

#[allow(clippy::too_many_arguments)]
fn act(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    wants: Res<EguiWantsInput>,
    mut world: ResMut<VoxelWorld>,
    mut sites: ResMut<Sites>,
    mut carry: ResMut<Carry>,
    mut dirty: ResMut<DirtyCells>,
    mut game: ResMut<Game>,
    mut sim: ResMut<Sim>,
) {
    if wants.wants_any_keyboard_input() || !keys.just_pressed(KeyCode::KeyF) {
        return;
    }
    let Some(target) = carry.target else { return };
    // A little dust where a block of stone is set down or lifted.
    let foot = |cell: IVec3| cell.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
    let dust = match target {
        Target::Put(cell) | Target::Take(cell) => Some(foot(cell)),
        Target::PutIn(Slot::Frame, holds) => Some(foot(sites.channel_well + IVec3::Y * (holds as i32 - 1))),
        Target::TakeOut(Slot::Frame, holds) => Some(foot(sites.channel_well + IVec3::Y * holds as i32)),
        _ => None,
    };
    if let Some(ledge) = apply(&mut world, &mut sites, &mut carry, &mut dirty, &mut game, target) {
        sim.request = Some(Request::DropStone(ledge));
    }
    if let Some(at) = dust {
        crate::particles::burst(&mut commands, crate::particles::Kind::Dust, at, 1.0);
    }
}

/// Do what a target says. Returns `Some(ledge)` when a quarry drop-stone was released so the caller
/// can trigger the timed fall in `sim`. Also used by the scripted tour.
pub fn apply(world: &mut VoxelWorld, sites: &mut Sites, carry: &mut Carry, dirty: &mut DirtyCells, game: &mut Game, target: Target) -> Option<DropLedge> {
    let mut dropped = None;
    match target {
        Target::Take(cell) => {
            let i = carry.loose.iter().position(|c| *c == cell)?;
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
        Target::TakeStone => carry.holding = Some(Item::QuarryStone),
        Target::StowStone => carry.holding = None,
        Target::DropStone(ledge) => {
            carry.holding = None;
            dropped = Some(ledge);
        }
        Target::PutIn(slot, holds) | Target::TakeOut(slot, holds) => {
            set_slot(world, sites, dirty, slot, holds);
            carry.filled[slot.index()] = holds;
            carry.holding = matches!(target, Target::TakeOut(..)).then_some(slot.takes());
            let puzzle = game.puzzle_of(slot.puzzle());
            let id = game.def(puzzle).id.clone();
            game.save.placed.insert(id, holds);
            game.persist();
        }
        Target::PutOnBeam(arm, seat) | Target::TakeOffBeam(arm, seat) => {
            let on = carry.beam.get(arm, seat);
            let put = matches!(target, Target::PutOnBeam(..));
            carry.beam.set(arm, seat, if put { on + 1 } else { on.saturating_sub(1) });
            carry.holding = (!put).then_some(Item::Block(Yard::Beam));
            game.save.sides.entry(crate::beam::BEAM_ID.to_string()).or_default().placed = carry.beam.to_saved();
            game.persist();
        }
        Target::Blocked(_) => return None,
    }
    carry.target = None;
    dropped
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

/// A carried block or quarry stone rides in front of the avatar, between its hands.
#[allow(clippy::type_complexity)]
fn hold(
    carry: Res<Carry>,
    player: Query<&Transform, (With<Player>, Without<HeldBlock>, Without<HeldStone>)>,
    mut held_block: Single<(&mut Transform, &mut Visibility), (With<HeldBlock>, Without<HeldStone>)>,
    mut held_stone: Single<(&mut Transform, &mut Visibility), (With<HeldStone>, Without<HeldBlock>)>,
) {
    let Ok(avatar) = player.single() else { return };
    let pose = Transform::from_translation(avatar.translation + avatar.rotation * HELD_AT).with_rotation(avatar.rotation);
    *held_block.0 = pose;
    *held_block.1 = if matches!(carry.holding, Some(Item::Block(_))) { Visibility::Inherited } else { Visibility::Hidden };
    *held_stone.0 = pose;
    *held_stone.1 = if carry.holding == Some(Item::QuarryStone) { Visibility::Inherited } else { Visibility::Hidden };
}

/// Outline where `F` would act: the block to lift, the ghost of the one to put down, or the quarry
/// stone poised at a drop lip.
fn draw_target(carry: Res<Carry>, sites: Res<Sites>, mut gizmos: Gizmos) {
    let Some(target) = carry.target else { return };
    let take = Color::srgb(1.0, 0.85, 0.3);
    let put = Color::srgb(0.4, 0.9, 1.0);
    let cube = |gizmos: &mut Gizmos, centre: Vec3, size: Vec3, colour: Color| gizmos.cube(Transform::from_translation(centre).with_scale(size), colour);
    match target {
        Target::Take(cell) => cube(&mut gizmos, cell.as_vec3() + Vec3::splat(0.5), Vec3::splat(1.04), take),
        Target::Put(cell) => cube(&mut gizmos, cell.as_vec3() + Vec3::splat(0.5), Vec3::splat(1.04), put),
        Target::PutIn(Slot::Frame, holds) => {
            cube(&mut gizmos, (sites.channel_well + IVec3::Y * (holds as i32 - 1)).as_vec3() + Vec3::splat(0.5), Vec3::splat(1.04), put)
        }
        Target::TakeOut(Slot::Frame, left) => {
            cube(&mut gizmos, (sites.channel_well + IVec3::Y * left as i32).as_vec3() + Vec3::splat(0.5), Vec3::splat(1.04), take)
        }
        Target::PutIn(Slot::Cart, _) => cube(&mut gizmos, sites.cart + Vec3::Y * (0.7 + HELD_SIZE / 2.0), Vec3::splat(HELD_SIZE + 0.04), put),
        Target::TakeOut(Slot::Cart, _) => cube(&mut gizmos, sites.cart + Vec3::Y * (0.7 + HELD_SIZE / 2.0), Vec3::splat(HELD_SIZE + 0.04), take),
        Target::DropStone(ledge) => {
            let release = ledge.release(&sites);
            let landing = ledge.landing(&sites);
            cube(&mut gizmos, release + Vec3::Y * (QUARRY_STONE_SIZE / 2.0), Vec3::splat(QUARRY_STONE_SIZE + 0.04), put);
            gizmos.line(release, landing, Color::srgba(0.4, 0.9, 1.0, 0.55));
        }
        Target::TakeStone => cube(&mut gizmos, sites.stone_pile + Vec3::Y * 0.6, Vec3::new(1.1, 1.1, 1.1), take),
        Target::StowStone => cube(&mut gizmos, sites.stone_pile + Vec3::Y * 0.6, Vec3::new(1.1, 1.1, 1.1), put),
        Target::PutOnBeam(arm, seat) => {
            let at = sites.beam.seat(arm, seat) + Vec3::Y * (HELD_SIZE * (carry.beam.get(arm, seat) as f32 + 0.5));
            cube(&mut gizmos, at, Vec3::splat(HELD_SIZE + 0.04), put);
        }
        Target::TakeOffBeam(arm, seat) => {
            let at = sites.beam.seat(arm, seat) + Vec3::Y * (HELD_SIZE * (carry.beam.get(arm, seat) as f32 - 0.5));
            cube(&mut gizmos, at, Vec3::splat(HELD_SIZE + 0.04), take);
        }
        Target::Blocked(_) => {}
    }
}

/// The beam's arms as the hints name them.
fn arm_name(arm: Arm) -> &'static str {
    match arm {
        Arm::Load => "load's arm (west)",
        Arm::Far => "far arm (east)",
    }
}

/// The line the hints bar shows for what `F` would do.
pub fn hint(carry: &Carry) -> Option<String> {
    Some(match carry.target? {
        Target::Take(_) => "F: pick up the block".into(),
        Target::Put(_) => "F: put the block down".into(),
        Target::TakeStone => "F: pick up a quarry stone".into(),
        Target::StowStone => "F: put the quarry stone back on the stack".into(),
        Target::DropStone(DropLedge::Low) => "F: drop the stone off the low step (Lumen will time the fall)".into(),
        Target::DropStone(DropLedge::Cliff) => "F: drop the stone off the cliff rim (Lumen will time the fall)".into(),
        Target::PutIn(Slot::Frame, _) => "F: slide the block into the jack frame (the launcher goes up a metre)".into(),
        Target::TakeOut(Slot::Frame, _) => "F: take a block out of the jack frame (the launcher comes down a metre)".into(),
        Target::PutIn(Slot::Cart, _) => "F: load the block onto the cart".into(),
        Target::TakeOut(Slot::Cart, _) => "F: take the block off the cart".into(),
        Target::PutOnBeam(arm, seat) => format!("F: put the block on seat {seat} of the beam's {}", arm_name(arm)),
        Target::TakeOffBeam(arm, seat) => format!("F: take a block off seat {seat} of the beam's {}", arm_name(arm)),
        Target::Blocked(why) => format!("{} {why}", if carry.holding == Some(Item::QuarryStone) { "Carrying a quarry stone." } else { "Carrying a block." }),
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
            let Some(Target::Take(cell)) = target_for(&world, &sites, &carry, feet, Vec3::X) else { panic!("seed {seed}: nothing to take") };
            assert_eq!(cell.y, base.y as i32 + 1, "the top block is taken first");
            assert_eq!(yard_of(&sites, cell), Yard::Ledge);
            world.set(cell, Material::Air);
            carry.loose.retain(|c| *c != cell);
            carry.holding = Some(Item::Block(Yard::Ledge));

            // At the frame's foot on the launcher's rail, facing it.
            let out = sites.channel_launch.direction;
            let foot = sites.channel_well.as_vec3() + Vec3::new(0.5, 0.0, 0.5) - out * 1.4;
            assert_eq!(target_for(&world, &sites, &carry, foot, out), Some(Target::PutIn(Slot::Frame, 1)), "seed {seed}");
            // Facing away from it, the block cannot be put on the rail.
            assert!(matches!(target_for(&world, &sites, &carry, foot, -out), Some(Target::Blocked(_))), "seed {seed}");

            set_stack(&mut world, &mut sites, &mut dirty, 3);
            assert_eq!(sites.channel_launch.origin, base + Vec3::Y * 3.0);
            assert_eq!(sites.pins[sites.channel_release_pin].position, sites.channel_launch.origin);
            assert!(world.is_solid(sites.channel_well + IVec3::Y * 2));
            assert!(!world.is_solid(sites.channel_well + IVec3::Y * 3));
            carry.filled[Slot::Frame.index()] = 3;
            carry.holding = None;
            assert_eq!(target_for(&world, &sites, &carry, foot, out), Some(Target::TakeOut(Slot::Frame, 2)));
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
        assert_eq!(target_for(&world, &sites, &carry, feet, Vec3::Z), Some(Target::Put(IVec3::new(low.x, low.y, high.z - 1))));
        // Facing out of the yard (west, off its edge).
        assert!(matches!(target_for(&world, &sites, &carry, feet, Vec3::NEG_X), Some(Target::Blocked(_))));
        assert!(overlaps_avatar(feet, feet.floor().as_ivec3()));
        // A ledge block does not go into the rail's yard, nor onto the cart.
        let [rail_low, _] = sites.cargo_yard;
        let by_rail = Vec3::new(rail_low.x as f32 + 0.5, rail_low.y as f32, rail_low.z as f32 - 0.6);
        assert!(matches!(target_for(&world, &sites, &carry, by_rail, Vec3::Z), Some(Target::Blocked(_))));
    }

    #[test]
    fn the_carts_block_is_loaded_onto_the_cart() {
        for seed in [0, 7, 42, 199] {
            let (mut world, sites, mut carry) = laid_out(seed);
            // From beside the block, facing it.
            let cell = sites.cargo_cell;
            let feet = Vec3::new(cell.x as f32 - 0.6, cell.y as f32, cell.z as f32 + 0.5);
            assert_eq!(target_for(&world, &sites, &carry, feet, Vec3::X), Some(Target::Take(cell)), "seed {seed}");
            world.set(cell, Material::Air);
            carry.loose.retain(|c| *c != cell);
            carry.holding = Some(Item::Block(yard_of(&sites, cell)));
            assert_eq!(carry.holding, Some(Item::Block(Yard::Rail)));
            // Facing the cart from the rail behind it.
            let behind = sites.cart - Vec3::X * 1.2;
            assert_eq!(target_for(&world, &sites, &carry, behind, Vec3::X), Some(Target::PutIn(Slot::Cart, 1)), "seed {seed}");
            // Back down in its yard instead.
            assert_eq!(target_for(&world, &sites, &carry, feet, Vec3::X), Some(Target::Put(cell)), "seed {seed}");
            // A block on the cart comes off it again.
            carry.holding = None;
            carry.filled[Slot::Cart.index()] = 1;
            assert_eq!(target_for(&world, &sites, &carry, behind, Vec3::X), Some(Target::TakeOut(Slot::Cart, 0)));
        }
    }

    #[test]
    fn the_beams_blocks_go_on_its_seats_and_come_off_again() {
        for seed in [0, 7, 42, 199] {
            let (mut world, sites, mut carry) = laid_out(seed);
            let beam = sites.beam.clone();
            for cell in &beam.pile {
                world.set(*cell, Material::Limestone);
                carry.loose.push(*cell);
            }
            // A block from the pile is the beam's.
            let top = *beam.pile.iter().max_by_key(|c| (c.y, c.z, c.x)).unwrap();
            assert_eq!(yard_of(&sites, top), Yard::Beam);
            carry.holding = Some(Item::Block(Yard::Beam));
            // South of the beam, facing it, in front of seat 3 of the far arm.
            let feet = Vec3::new(beam.pivot.x + 3.0, beam.pivot.y - crate::island::BEAM_AXLE, beam.pivot.z + 1.4);
            assert_eq!(target_for(&world, &sites, &carry, feet, Vec3::NEG_Z), Some(Target::PutOnBeam(Arm::Far, 3)), "seed {seed}");
            // And from the north side, in front of seat 2 of the load's arm.
            let north = Vec3::new(beam.pivot.x - 2.0, feet.y, beam.pivot.z - 1.4);
            assert_eq!(target_for(&world, &sites, &carry, north, Vec3::Z), Some(Target::PutOnBeam(Arm::Load, 2)), "seed {seed}");
            // Over the pivot, nothing goes.
            let pivot = Vec3::new(beam.pivot.x, feet.y, feet.z);
            assert!(matches!(target_for(&world, &sites, &carry, pivot, Vec3::NEG_Z), Some(Target::Blocked(_))));
            // Facing away from it, the block goes down by the pile or nowhere.
            assert!(!matches!(target_for(&world, &sites, &carry, feet, Vec3::Z), Some(Target::PutOnBeam(..))));

            let mut dirty = DirtyCells::default();
            let mut game = Game::for_test(seed);
            apply(&mut world, &mut sites.clone(), &mut carry, &mut dirty, &mut game, Target::PutOnBeam(Arm::Far, 3));
            assert_eq!(carry.beam.get(Arm::Far, 3), 1);
            assert!(carry.holding.is_none());
            assert_eq!(Placement::from_saved(&game.save.side(crate::beam::BEAM_ID).placed), carry.beam, "the save keeps it");
            // Empty-handed, the top block comes off again.
            assert_eq!(target_for(&world, &sites, &carry, feet, Vec3::NEG_Z), Some(Target::TakeOffBeam(Arm::Far, 3)));
            // Not while the beam swings.
            carry.beam_swinging = true;
            assert!(matches!(target_for(&world, &sites, &carry, feet, Vec3::NEG_Z), Some(Target::Blocked(_))));
            carry.beam_swinging = false;
            // A full seat takes no more.
            carry.beam.set(Arm::Far, 3, MOST_ON_A_SEAT);
            carry.holding = Some(Item::Block(Yard::Beam));
            assert_eq!(target_for(&world, &sites, &carry, feet, Vec3::NEG_Z), Some(Target::Blocked("That seat is full.")));
            let _ = std::fs::remove_file(&game.save_path);
        }
    }

    #[test]
    fn quarry_stones_are_picked_up_and_dropped_off_either_ledge() {
        for seed in [0, 7, 42, 199] {
            let (world, sites, mut carry) = laid_out(seed);
            // Pick up from the stack beside the console.
            let at_pile = sites.stone_pile - Vec3::X * 1.1;
            assert_eq!(target_for(&world, &sites, &carry, at_pile, Vec3::X), Some(Target::TakeStone), "seed {seed}");
            carry.holding = Some(Item::QuarryStone);
            assert_eq!(target_for(&world, &sites, &carry, at_pile, Vec3::X), Some(Target::StowStone));
            // Stand on the 1 m low step facing south toward the drop lip.
            let at_low = sites.low_drop - Vec3::Z * 1.0;
            assert_eq!(target_for(&world, &sites, &carry, at_low, Vec3::Z), Some(Target::DropStone(DropLedge::Low)), "seed {seed}");
            // Stand on the cliff rim facing south toward the drop lip.
            let at_cliff = sites.cliff_drop - Vec3::Z * 1.0;
            assert_eq!(target_for(&world, &sites, &carry, at_cliff, Vec3::Z), Some(Target::DropStone(DropLedge::Cliff)), "seed {seed}");
            // Facing north (away from the drop lip), it refuses to drop.
            assert!(matches!(target_for(&world, &sites, &carry, at_cliff, Vec3::NEG_Z), Some(Target::Blocked(_))));
        }
    }

    #[test]
    fn placement_badges_report_quarry_drops_stacks_and_beam_seats() {
        let (_, sites, mut carry) = laid_out(7);
        carry.target = Some(Target::DropStone(DropLedge::Cliff));
        assert_eq!(badge_for(&carry, &sites).map(|(_, t, put)| (t, put)), Some(("Drop stone (cliff rim)".into(), true)));

        carry.target = Some(Target::PutIn(Slot::Frame, 5));
        assert_eq!(badge_for(&carry, &sites).map(|(_, t, put)| (t, put)), Some(("5 m (5 blocks)".into(), true)));

        carry.filled[Slot::Frame.index()] = 5;
        carry.target = Some(Target::TakeOut(Slot::Frame, 4));
        assert_eq!(badge_for(&carry, &sites).map(|(_, t, put)| (t, put)), Some(("Stack: 5 m → 4 m".into(), false)));

        carry.target = Some(Target::PutOnBeam(Arm::Far, 3));
        assert_eq!(badge_for(&carry, &sites).map(|(_, t, put)| (t, put)), Some(("Seat 3 (1)".into(), true)));
    }
}
