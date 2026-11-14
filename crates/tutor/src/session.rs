//! A conversation with the tutor. Requests run on their own thread; the game polls for text.

use crate::context::TutorContext;
use crate::filter::LeakFilter;
use crate::gesture::{strip_looks, strip_looks_streaming};
use crate::prompt::system_prompt;
use crate::provider::{ChatRequest, Message, Provider, Role, TutorError};
use crate::settings::TutorSettings;
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::sync::{Arc, Mutex};

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
    /// In a mutex only so that a conversation can be shared between threads; one thread polls it.
    events: Mutex<Receiver<Event>>,
    raw: String,
    filter: LeakFilter,
    /// Places the reply may point at, and how many of its look tags have been passed on so far.
    landmarks: Vec<String>,
    looks_seen: usize,
}

/// How many past turns are sent with each request.
const HISTORY_TURNS: usize = 24;

#[derive(Default)]
pub struct Conversation {
    pub turns: Vec<Turn>,
    pending: Option<Pending>,
    /// The latest place the tutor pointed at that the game has not yet taken.
    look: Option<String>,
}

impl Conversation {
    pub fn is_waiting(&self) -> bool {
        self.pending.is_some()
    }

    /// The reply so far, filtered and without look tags, while one is arriving.
    pub fn arriving(&self) -> Option<String> {
        self.pending.as_ref().map(|p| p.filter.apply_streaming(&strip_looks_streaming(&p.raw, &p.landmarks).0))
    }

    /// The place the tutor last pointed at with a `[look: NAME]` tag, once. Names are always ones
    /// from the snapshot's landmarks. A tag counts as soon as it has arrived in full, so the eye can
    /// turn while the reply is still being written.
    pub fn take_look(&mut self) -> Option<String> {
        self.look.take()
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
        self.pending = Some(Pending {
            events: Mutex::new(events),
            raw: String::new(),
            filter: LeakFilter::for_context(ctx, &player_text),
            landmarks: ctx.landmarks.clone(),
            looks_seen: 0,
        });
        true
    }

    /// Collect whatever has arrived. Call once per frame. Returns `true` if anything changed.
    pub fn poll(&mut self) -> bool {
        let Some(pending) = &mut self.pending else { return false };
        let mut changed = false;
        let events = pending.events.get_mut().unwrap_or_else(|poisoned| poisoned.into_inner());
        let finished = loop {
            match events.try_recv() {
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
        let mut look = None;
        if changed {
            let (_, looks) = strip_looks_streaming(&pending.raw, &pending.landmarks);
            if looks.len() > pending.looks_seen {
                pending.looks_seen = looks.len();
                look = looks.last().cloned();
            }
        }
        if look.is_some() {
            self.look = look;
        }
        let Some(result) = finished else { return changed };
        let pending = self.pending.take().expect("checked above");
        let (text, looks) = strip_looks(&pending.raw, &pending.landmarks);
        if looks.len() > pending.looks_seen {
            self.look = looks.last().cloned();
        }
        let reply = pending.filter.apply(&text);
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
