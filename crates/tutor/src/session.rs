//! A conversation with the tutor. Requests run on their own thread; the game polls for text.

use crate::context::TutorContext;
use crate::filter::LeakFilter;
use crate::prompt::system_prompt;
use crate::provider::{ChatRequest, Message, Provider, Role, TutorError};
use crate::settings::TutorSettings;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

/// The configured tutor, or the reason there is none.
#[derive(Clone)]
pub struct Tutor {
    provider: Option<Arc<dyn Provider>>,
    status: String,
}

impl Tutor {
    pub fn offline(reason: impl Into<String>) -> Self {
        Tutor { provider: None, status: reason.into() }
    }

    pub fn with_provider(provider: Arc<dyn Provider>) -> Self {
        let status = provider.describe();
        Tutor { provider: Some(provider), status }
    }

    /// Set up from a settings file. Never fails: any problem leaves the tutor offline with the
    /// reason as its status.
    pub fn from_settings_file(path: &Path) -> Self {
        match TutorSettings::load(path) {
            Ok(None) => Tutor::offline(format!("No [tutor] section in {}.", path.display())),
            Ok(Some(settings)) => match settings.provider() {
                Ok(provider) => Tutor::with_provider(provider),
                Err(e) => Tutor::offline(e.to_string()),
            },
            Err(e) => Tutor::offline(e.to_string()),
        }
    }

    pub fn is_online(&self) -> bool {
        self.provider.is_some()
    }

    /// The service in use ("model at host"), or why the tutor is offline.
    pub fn status(&self) -> &str {
        &self.status
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Turn {
    Player(String),
    Tutor(String),
    /// A request that failed, with the reason shown to the player.
    Problem(String),
}

enum Event {
    Text(String),
    Done,
    Failed(TutorError),
}

struct Pending {
    events: Receiver<Event>,
    raw: String,
    filter: LeakFilter,
}

/// How many past turns are sent with each request.
const HISTORY_TURNS: usize = 24;

#[derive(Default)]
pub struct Conversation {
    pub turns: Vec<Turn>,
    pending: Option<Pending>,
}

impl Conversation {
    pub fn is_waiting(&self) -> bool {
        self.pending.is_some()
    }

    /// The reply so far, filtered, while one is arriving.
    pub fn arriving(&self) -> Option<String> {
        self.pending.as_ref().map(|p| p.filter.apply_streaming(&p.raw))
    }

    /// Send the player's message with a fresh snapshot. Returns `false` if nothing was sent
    /// (offline, already waiting, or an empty message).
    pub fn ask(&mut self, tutor: &Tutor, ctx: &TutorContext, text: &str) -> bool {
        let text = text.trim();
        let Some(provider) = tutor.provider.clone() else { return false };
        if text.is_empty() || self.pending.is_some() {
            return false;
        }
        self.turns.push(Turn::Player(text.to_string()));

        let player_text: String =
            self.turns.iter().filter_map(|t| if let Turn::Player(s) = t { Some(s.as_str()) } else { None }).collect::<Vec<_>>().join("\n");
        let request = ChatRequest { system: system_prompt(ctx), messages: self.history() };
        let (sender, events) = channel();
        std::thread::Builder::new()
            .name("tutor-request".into())
            .spawn(move || {
                let result = provider.stream_chat(&request, &mut |piece| sender.send(Event::Text(piece.to_string())).is_ok());
                let _ = sender.send(match result {
                    Ok(()) => Event::Done,
                    Err(e) => Event::Failed(e),
                });
            })
            .expect("a thread can be started");
        self.pending = Some(Pending { events, raw: String::new(), filter: LeakFilter::for_context(ctx, &player_text) });
        true
    }

    /// Collect whatever has arrived. Call once per frame. Returns `true` if anything changed.
    pub fn poll(&mut self) -> bool {
        let Some(pending) = &mut self.pending else { return false };
        let mut changed = false;
        let finished = loop {
            match pending.events.try_recv() {
                Ok(Event::Text(piece)) => {
                    pending.raw.push_str(&piece);
                    changed = true;
                }
                Ok(Event::Done) => break Some(Ok(())),
                Ok(Event::Failed(e)) => break Some(Err(e.to_string())),
                Err(TryRecvError::Empty) => break None,
                Err(TryRecvError::Disconnected) => break Some(Err("The request ended unexpectedly.".to_string())),
            }
        };
        let Some(result) = finished else { return changed };
        let pending = self.pending.take().expect("checked above");
        let reply = pending.filter.apply(&pending.raw);
        if !reply.trim().is_empty() {
            self.turns.push(Turn::Tutor(reply.trim().to_string()));
        }
        if let Err(problem) = result {
            self.turns.push(Turn::Problem(problem));
        }
        true
    }

    /// Past turns as the service expects them: alternating, starting with the player.
    fn history(&self) -> Vec<Message> {
        let mut messages: Vec<Message> = Vec::new();
        let recent = &self.turns[self.turns.len().saturating_sub(HISTORY_TURNS)..];
        for turn in recent {
            let (role, content) = match turn {
                Turn::Player(s) => (Role::User, s),
                Turn::Tutor(s) => (Role::Assistant, s),
                Turn::Problem(_) => continue,
            };
            match messages.last_mut() {
                // Two messages in a row from one side (after a failed request) are merged.
                Some(last) if last.role == role => {
                    last.content.push_str("\n\n");
                    last.content.push_str(content);
                }
                _ => messages.push(Message { role, content: content.clone() }),
            }
        }
        if messages.first().is_some_and(|m| m.role == Role::Assistant) {
            messages.remove(0);
        }
        messages
    }
}
