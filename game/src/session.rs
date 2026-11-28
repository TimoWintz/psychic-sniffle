//! Session lifecycle: building the save-dependent world at startup and rebuilding it cleanly when
//! switching save slots, starting a new game, or returning to the main menu.

use crate::intro::Intro;
use crate::state::{BuildWorld, Game, Mode, SessionAction, SessionControl, Ui, WorldEntity, fresh_seed};
use bevy::prelude::*;

pub struct SessionPlugin;

impl Plugin for SessionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SessionControl>()
            .add_systems(PostStartup, run_initial_world_build)
            .add_systems(OnEnter(Mode::Playing), mark_dirty)
            .add_systems(First, process_actions);
    }
}

fn run_initial_world_build(world: &mut World) {
    world.run_schedule(BuildWorld);
}

fn mark_dirty(mut control: ResMut<SessionControl>) {
    control.world_dirty = true;
}

/// Process deferred save load / new game / return-to-menu requests in `First` (before
/// `StateTransition`) so returning to the menu never flashes a Playing frame.
fn process_actions(world: &mut World) {
    let Some(action) = world.resource_mut::<SessionControl>().pending.take() else { return };
    match action {
        SessionAction::ReturnToMenu => {
            world.resource_mut::<NextState<Mode>>().set(Mode::Menu);
        }
        SessionAction::LoadAndPlay { path } => {
            let same_clean = !world.resource::<SessionControl>().world_dirty && world.resource::<Game>().save_path == path;
            if !same_clean {
                let game = Game::from_path(path);
                rebuild_world(world, game);
            }
            world.resource_mut::<SessionControl>().world_dirty = false;
            world.resource_mut::<Intro>().request_start();
        }
        SessionAction::NewGameAndPlay { path } => {
            let game = Game::new_at(path, fresh_seed());
            rebuild_world(world, game);
            world.resource_mut::<SessionControl>().world_dirty = false;
            world.resource_mut::<Intro>().request_start();
        }
    }
}

/// Despawn every save-dependent entity, replace `Game`/`VoxelWorld`/`Sites`, reset session
/// resources, and re-run [`BuildWorld`].
fn rebuild_world(world: &mut World, game: Game) {
    let entities: Vec<Entity> = world.query_filtered::<Entity, With<WorldEntity>>().iter(world).collect();
    for entity in entities {
        let _ = world.despawn(entity);
    }

    let (mut voxel_world, sites) = crate::island::build(&game.layout);
    let stage = crate::waking::stage(&game);
    crate::waking::settle(&mut voxel_world, &sites.way, stage);
    crate::beam::settle(&mut voxel_world, &sites.beam, crate::beam::woken(&game));

    world.insert_resource(game);
    world.insert_resource(voxel_world);
    world.insert_resource(sites);
    crate::waking::reset_resources(world, stage);
    world.insert_resource(Ui::default());
    world.insert_resource(crate::ruler::Ruler::default());
    world.insert_resource(crate::sim::Sim::default());
    world.insert_resource(crate::sim::TerrainReshaped::default());

    world.run_schedule(BuildWorld);
}
