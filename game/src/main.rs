//! First Principles: a physics game. See `docs/` for the design.

mod avatar;
mod carry;
mod cinematic;
mod companion;
mod devscript;
mod handbook;
mod intro;
mod island;
mod player;
mod props;
mod rig;
mod ruler;
mod scenery;
mod shipyard;
mod sim;
mod state;
mod stations;
mod tutor_bridge;
mod ui;
mod voxel;

use avian3d::prelude::PhysicsPlugins;
use bevy::prelude::*;
use bevy_egui::EguiPlugin;
use state::Game;

fn main() {
    let game = Game::load_or_new();
    let (world, sites) = island::build(&game.layout);

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window { title: "First Principles".into(), resolution: (1920, 1080).into(), ..default() }),
            ..default()
        }))
        .add_plugins((EguiPlugin::default(), PhysicsPlugins::default()))
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
            shipyard::ShipyardPlugin,
            tutor_bridge::TutorPlugin,
            ui::UiPlugin,
            companion::CompanionPlugin,
            devscript::DevScriptPlugin,
        ))
        .run();
}
