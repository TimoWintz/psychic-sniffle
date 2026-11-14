//! The one-way bridge to the tutor.
//!
//! The game copies a snapshot of the current puzzle into the tutor crate and shows the text that
//! comes back. Nothing the tutor returns is interpreted: it cannot change the game.

use crate::companion::Landmarks;
use crate::state::{Game, PUZZLES, Ui};
use bevy::prelude::*;
use std::path::PathBuf;
use tutor::{Conversation, Tutor, TutorContext};

#[derive(Resource)]
pub struct Lumen {
    pub tutor: Tutor,
    /// One conversation per puzzle, so advice about one does not leak into another.
    pub conversations: [Conversation; PUZZLES],
}

impl Lumen {
    /// Send the player's message about `puzzle`, with a fresh snapshot of its state and the names of
    /// the places Lumen can point at.
    pub fn ask(&mut self, game: &Game, ui: &Ui, landmarks: &Landmarks, puzzle: usize, text: &str) -> bool {
        let mut context = TutorContext::build(&game.chapter, game.def(puzzle), &game.instance(puzzle), &game.save, &ui.drafts[puzzle]);
        context.landmarks = landmarks.names();
        self.conversations[puzzle].ask(&self.tutor, &context, text)
    }
}

pub struct TutorPlugin;

impl Plugin for TutorPlugin {
    fn build(&self, app: &mut App) {
        // `GAME_SETTINGS` overrides where the settings file is looked for.
        let path = PathBuf::from(std::env::var("GAME_SETTINGS").unwrap_or_else(|_| "settings.toml".into()));
        app.insert_resource(Lumen { tutor: Tutor::from_settings_file(&path), conversations: Default::default() })
            .add_systems(Update, poll);
    }
}

fn poll(mut lumen: ResMut<Lumen>) {
    // Only mark the resource changed when text actually arrived.
    if lumen.conversations.iter().any(Conversation::is_waiting) {
        for conversation in &mut lumen.conversations {
            conversation.poll();
        }
    }
}
