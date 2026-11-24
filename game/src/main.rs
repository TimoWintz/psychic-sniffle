//! First Principles: a physics game. See `docs/` for the design.

mod avatar;
mod carry;
mod cinematic;
mod companion;
mod devscript;
mod graphics;
mod handbook;
mod intro;
mod island;
mod player;
mod prefill;
mod props;
mod rig;
mod ruler;
mod scenery;
mod sim;
mod state;
mod stations;
mod tutor_bridge;
mod ui;
mod vessel;
mod voxel;
mod waking;

use avian3d::prelude::PhysicsPlugins;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use state::Game;

fn main() {
    let game = Game::load_or_new();
    let (mut world, sites) = island::build(&game.layout);
    // The island is built awake: put the way to the stage the save is at before anything meshes it
    // or collides with it.
    waking::settle(&mut world, &sites.way, waking::stage(&game));

    App::new()
        .add_plugins(
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window { title: "First Principles".into(), resolution: (1920, 1080).into(), ..default() }),
                ..default()
            }),
        )
        .add_plugins((EguiPlugin::default(), PhysicsPlugins::default(), graphics::GraphicsPlugin))
        .insert_resource(game)
        .insert_resource(world)
        .insert_resource(sites)
        .add_plugins((
            // First: it sets up the menu state the other plugins' systems wait on.
            intro::IntroPlugin,
            scenery::SceneryPlugin,
            player::PlayerPlugin,
            avatar::AvatarPlugin,
            stations::StationsPlugin,
            sim::SimPlugin,
            carry::CarryPlugin,
            rig::RigPlugin,
            ruler::RulerPlugin,
            vessel::VesselPlugin,
            tutor_bridge::TutorPlugin,
            ui::UiPlugin,
            companion::CompanionPlugin,
            waking::WakingPlugin,
            devscript::DevScriptPlugin,
        ))
        .run();
}
