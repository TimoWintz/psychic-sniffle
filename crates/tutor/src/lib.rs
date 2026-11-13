//! The read-only tutor.
//!
//! This crate does not depend on the game engine. It receives a [`TutorContext`] — a plain copy of
//! puzzle state — and produces text. It has no way to refer to, let alone change, anything in the
//! game; that is what "the tutor cannot act" rests on.

pub mod context;
pub mod filter;
pub mod prompt;
pub mod provider;
pub mod session;
pub mod settings;

pub use context::TutorContext;
pub use provider::{Provider, TutorError};
pub use session::{Conversation, Turn, Tutor};
